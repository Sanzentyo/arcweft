//! Deep Copy admission for literals embedded in an unevaluated expression.

use super::{RuntimeExpr, RuntimeExprKind, RuntimeExpressionNode};
use crate::pattern::RuntimePatternKind;

impl RuntimeExpr {
    /// An unevaluated expression may be copied into an inert control image
    /// only when every embedded live value is recursively unrestricted.
    /// Local reads are evaluated later and do not duplicate their bindings.
    pub(crate) fn literals_permit_copy(&self) -> bool {
        self.try_visit_owned_tree(&mut |_, node| {
            let literal = match node {
                RuntimeExpressionNode::Expression(expression) => match expression.kind() {
                    RuntimeExprKind::Value(value) => Some(value),
                    _ => None,
                },
                RuntimeExpressionNode::Pattern(pattern) => match pattern.kind() {
                    RuntimePatternKind::Literal(value) => Some(value),
                    _ => None,
                },
            };
            if literal.is_some_and(|value| !value.ownership().permits_copy()) {
                Err(())
            } else {
                Ok(())
            }
        })
        .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimePattern;
    use crate::runtime_id::RuntimePlanTypeId;
    use crate::value::RuntimeValue;
    use std::num::NonZeroU32;

    #[test]
    fn nested_affine_literal_does_not_admit_an_expression_copy() {
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        let expression = RuntimeExpr::from_admitted_parts(
            ty,
            RuntimeExprKind::Tuple(vec![
                RuntimeExpr::from_admitted_parts(ty, RuntimeExprKind::Value(RuntimeValue::i64(7))),
                RuntimeExpr::from_admitted_parts(
                    ty,
                    RuntimeExprKind::Value(RuntimeValue::NeedHandle(crate::tests::reusable_need(
                        "need.one",
                    ))),
                ),
            ]),
        );
        assert!(!expression.literals_permit_copy());

        let copy =
            RuntimeExpr::from_admitted_parts(ty, RuntimeExprKind::Value(RuntimeValue::i64(7)));
        let match_expression = RuntimeExpr::from_admitted_parts(
            ty,
            RuntimeExprKind::IfLet {
                pattern: RuntimePattern::from_admitted_parts(
                    ty,
                    RuntimePatternKind::Literal(RuntimeValue::NeedHandle(
                        crate::tests::reusable_need("need.pattern"),
                    )),
                ),
                expr: Box::new(copy.clone()),
                guard: None,
                then_expr: Box::new(copy.clone()),
                else_expr: Box::new(copy),
            },
        );
        assert!(!match_expression.literals_permit_copy());
    }
}
