use super::{ExprNodeKey, TypeChecker};
use crate::types::TypeKind;
use arcweft_lang_syntax::{
    ast::common::TextRange,
    expr::{CallArg, Expr, Placeholder, collect_expr_source_ranges},
};
use std::{collections::HashMap, mem};

impl TypeChecker<'_> {
    pub(super) fn check_expr_with_expected_at_range(
        &mut self,
        expr: &Expr,
        expected: Option<&TypeKind>,
        source_range: TextRange,
    ) -> Option<TypeKind> {
        let key = ExprNodeKey::from_expr(expr);
        let previous = self.expression_source_ranges.insert(key, source_range);
        let ty = self.check_expr_with_expected(expr, expected);
        if let Some(previous) = previous {
            self.expression_source_ranges.insert(key, previous);
        } else {
            self.expression_source_ranges.remove(&key);
        }
        ty
    }

    pub(super) fn check_desugared_expr_with_authored_ranges(
        &mut self,
        authored: &Expr,
        desugared: &Expr,
        root_range: TextRange,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let mut ranges = HashMap::new();
        self.collect_desugared_expr_source_ranges(authored, desugared, &mut ranges);
        ranges.insert(ExprNodeKey::from_expr(desugared), root_range);
        self.with_temporary_expr_source_ranges(ranges, |this| {
            this.check_expr_with_expected(desugared, expected)
        })
    }

    pub(super) fn register_expr_source_ranges(
        &mut self,
        expr: &Expr,
        expr_source: Option<&str>,
        expr_range: Option<TextRange>,
    ) {
        let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
            return;
        };
        for source_range in collect_expr_source_ranges(expr, expr_source, expr_range) {
            self.expression_source_ranges.insert(
                ExprNodeKey::from_expr(source_range.expr()),
                source_range.range(),
            );
        }
    }

    pub(super) fn source_range_for_expr(&self, expr: &Expr) -> Option<TextRange> {
        self.expression_source_ranges
            .get(&ExprNodeKey::from_expr(expr))
            .copied()
    }

    fn with_temporary_expr_source_ranges<R>(
        &mut self,
        ranges: HashMap<ExprNodeKey, TextRange>,
        check: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = ranges
            .into_iter()
            .map(|(key, range)| (key, self.expression_source_ranges.insert(key, range)))
            .collect::<Vec<_>>();
        let result = check(self);
        for (key, previous_range) in previous.into_iter().rev() {
            if let Some(previous_range) = previous_range {
                self.expression_source_ranges.insert(key, previous_range);
            } else {
                self.expression_source_ranges.remove(&key);
            }
        }
        result
    }

    fn collect_desugared_expr_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) {
        if matches!(authored, Expr::Placeholder(Placeholder::PipeLeft)) {
            return;
        }
        if let Some(range) = self.source_range_for_expr(authored) {
            ranges.insert(ExprNodeKey::from_expr(desugared), range);
        }
        if self.collect_desugared_sequence_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_pair_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_call_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_single_child_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_range_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_record_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_block_value_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_if_source_ranges(authored, desugared, ranges)
            || self.collect_desugared_if_let_source_ranges(authored, desugared, ranges)
        {
            return;
        }
        self.collect_desugared_match_source_ranges(authored, desugared, ranges);
    }

    fn collect_desugared_sequence_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        match (authored, desugared) {
            (Expr::Tuple(authored_items), Expr::Tuple(desugared_items))
            | (Expr::BracketSeq(authored_items), Expr::BracketSeq(desugared_items)) => {
                self.collect_desugared_expr_slices_source_ranges(
                    authored_items,
                    desugared_items,
                    ranges,
                );
                true
            }
            _ => false,
        }
    }

    fn collect_desugared_pair_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        match (authored, desugared) {
            (
                Expr::ArrayRepeat {
                    value: authored_first,
                    len: authored_second,
                },
                Expr::ArrayRepeat {
                    value: desugared_first,
                    len: desugared_second,
                },
            )
            | (
                Expr::Index {
                    target: authored_first,
                    index: authored_second,
                },
                Expr::Index {
                    target: desugared_first,
                    index: desugared_second,
                },
            )
            | (
                Expr::Binary {
                    lhs: authored_first,
                    rhs: authored_second,
                    ..
                },
                Expr::Binary {
                    lhs: desugared_first,
                    rhs: desugared_second,
                    ..
                },
            ) => {
                self.collect_desugared_expr_source_ranges(authored_first, desugared_first, ranges);
                self.collect_desugared_expr_source_ranges(
                    authored_second,
                    desugared_second,
                    ranges,
                );
                true
            }
            _ => false,
        }
    }

    fn collect_desugared_call_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        match (authored, desugared) {
            (
                Expr::Call {
                    callee: authored_callee,
                    args: authored_args,
                },
                Expr::Call {
                    callee: desugared_callee,
                    args: desugared_args,
                },
            ) => {
                if same_expr_variant(authored_callee, desugared_callee) {
                    self.collect_desugared_expr_source_ranges(
                        authored_callee,
                        desugared_callee,
                        ranges,
                    );
                }
                self.collect_desugared_call_args_source_ranges(
                    authored_args,
                    desugared_args,
                    ranges,
                );
                true
            }
            (authored, Expr::Call { callee, .. }) if same_expr_variant(authored, callee) => {
                self.collect_desugared_expr_source_ranges(authored, callee, ranges);
                true
            }
            _ => false,
        }
    }

    fn collect_desugared_single_child_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        match (authored, desugared) {
            (Expr::Select(authored_select), Expr::Select(desugared_select)) => {
                self.collect_desugared_expr_source_ranges(
                    authored_select.target(),
                    desugared_select.target(),
                    ranges,
                );
            }
            (
                Expr::DialogueCall {
                    callee: authored_child,
                    ..
                },
                Expr::DialogueCall {
                    callee: desugared_child,
                    ..
                },
            )
            | (
                Expr::Try {
                    expr: authored_child,
                },
                Expr::Try {
                    expr: desugared_child,
                },
            )
            | (
                Expr::Await {
                    expr: authored_child,
                    ..
                },
                Expr::Await {
                    expr: desugared_child,
                    ..
                },
            )
            | (
                Expr::Closure {
                    body: authored_child,
                    ..
                },
                Expr::Closure {
                    body: desugared_child,
                    ..
                },
            )
            | (
                Expr::Unary {
                    expr: authored_child,
                    ..
                },
                Expr::Unary {
                    expr: desugared_child,
                    ..
                },
            ) => {
                self.collect_desugared_expr_source_ranges(authored_child, desugared_child, ranges);
            }
            _ => return false,
        }
        true
    }

    fn collect_desugared_range_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        let (
            Expr::Range {
                start: authored_start,
                end: authored_end,
                ..
            },
            Expr::Range {
                start: desugared_start,
                end: desugared_end,
                ..
            },
        ) = (authored, desugared)
        else {
            return false;
        };
        self.collect_optional_desugared_expr_source_ranges(
            authored_start.as_deref(),
            desugared_start.as_deref(),
            ranges,
        );
        self.collect_optional_desugared_expr_source_ranges(
            authored_end.as_deref(),
            desugared_end.as_deref(),
            ranges,
        );
        true
    }

    fn collect_desugared_record_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        match (authored, desugared) {
            (
                Expr::Record {
                    fields: authored_fields,
                    ..
                },
                Expr::Record {
                    fields: desugared_fields,
                    ..
                },
            )
            | (Expr::RecordLiteral(authored_fields), Expr::RecordLiteral(desugared_fields)) => {
                for ((_, authored_value), (_, desugared_value)) in
                    authored_fields.iter().zip(desugared_fields)
                {
                    self.collect_desugared_expr_source_ranges(
                        authored_value,
                        desugared_value,
                        ranges,
                    );
                }
                true
            }
            _ => false,
        }
    }

    fn collect_desugared_block_value_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        let (authored_value, desugared_value) = match (authored, desugared) {
            (
                Expr::Block {
                    value: authored, ..
                },
                Expr::Block {
                    value: desugared, ..
                },
            )
            | (
                Expr::ComputationBlock {
                    value: authored, ..
                },
                Expr::ComputationBlock {
                    value: desugared, ..
                },
            )
            | (
                Expr::MemoBlock {
                    value: authored, ..
                },
                Expr::MemoBlock {
                    value: desugared, ..
                },
            )
            | (
                Expr::NamedBlock {
                    value: authored, ..
                },
                Expr::NamedBlock {
                    value: desugared, ..
                },
            ) => (authored.as_deref(), desugared.as_deref()),
            _ => return false,
        };
        self.collect_optional_desugared_expr_source_ranges(authored_value, desugared_value, ranges);
        true
    }

    fn collect_desugared_if_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        let (
            Expr::If {
                condition: authored_condition,
                then_branch: authored_then,
                else_branch: authored_else,
            },
            Expr::If {
                condition: desugared_condition,
                then_branch: desugared_then,
                else_branch: desugared_else,
            },
        ) = (authored, desugared)
        else {
            return false;
        };
        self.collect_desugared_expr_source_ranges(authored_condition, desugared_condition, ranges);
        self.collect_desugared_expr_source_ranges(authored_then, desugared_then, ranges);
        self.collect_optional_desugared_expr_source_ranges(
            authored_else.as_deref(),
            desugared_else.as_deref(),
            ranges,
        );
        true
    }

    fn collect_desugared_if_let_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        let (
            Expr::IfLet {
                expr: authored_expr,
                guard: authored_guard,
                then_branch: authored_then,
                else_branch: authored_else,
                ..
            },
            Expr::IfLet {
                expr: desugared_expr,
                guard: desugared_guard,
                then_branch: desugared_then,
                else_branch: desugared_else,
                ..
            },
        ) = (authored, desugared)
        else {
            return false;
        };
        self.collect_desugared_expr_source_ranges(authored_expr, desugared_expr, ranges);
        self.collect_optional_desugared_expr_source_ranges(
            authored_guard.as_deref(),
            desugared_guard.as_deref(),
            ranges,
        );
        self.collect_desugared_expr_source_ranges(authored_then, desugared_then, ranges);
        self.collect_optional_desugared_expr_source_ranges(
            authored_else.as_deref(),
            desugared_else.as_deref(),
            ranges,
        );
        true
    }

    fn collect_desugared_match_source_ranges(
        &self,
        authored: &Expr,
        desugared: &Expr,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) -> bool {
        let (
            Expr::Match {
                scrutinee: authored_scrutinee,
                arms: authored_arms,
            },
            Expr::Match {
                scrutinee: desugared_scrutinee,
                arms: desugared_arms,
            },
        ) = (authored, desugared)
        else {
            return false;
        };
        self.collect_desugared_expr_source_ranges(authored_scrutinee, desugared_scrutinee, ranges);
        for (authored_arm, desugared_arm) in authored_arms.iter().zip(desugared_arms) {
            self.collect_optional_desugared_expr_source_ranges(
                authored_arm.guard(),
                desugared_arm.guard(),
                ranges,
            );
            self.collect_desugared_expr_source_ranges(
                authored_arm.value(),
                desugared_arm.value(),
                ranges,
            );
        }
        true
    }

    fn collect_desugared_expr_slices_source_ranges(
        &self,
        authored_items: &[Expr],
        desugared_items: &[Expr],
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) {
        for (authored, desugared) in authored_items.iter().zip(desugared_items) {
            self.collect_desugared_expr_source_ranges(authored, desugared, ranges);
        }
    }

    fn collect_desugared_call_args_source_ranges(
        &self,
        authored_args: &[CallArg],
        desugared_args: &[CallArg],
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) {
        for (authored_arg, desugared_arg) in authored_args.iter().zip(desugared_args) {
            self.collect_desugared_expr_source_ranges(
                authored_arg.value(),
                desugared_arg.value(),
                ranges,
            );
        }
    }

    fn collect_optional_desugared_expr_source_ranges(
        &self,
        authored: Option<&Expr>,
        desugared: Option<&Expr>,
        ranges: &mut HashMap<ExprNodeKey, TextRange>,
    ) {
        if let (Some(authored), Some(desugared)) = (authored, desugared) {
            self.collect_desugared_expr_source_ranges(authored, desugared, ranges);
        }
    }
}

fn same_expr_variant(lhs: &Expr, rhs: &Expr) -> bool {
    mem::discriminant(lhs) == mem::discriminant(rhs)
}
