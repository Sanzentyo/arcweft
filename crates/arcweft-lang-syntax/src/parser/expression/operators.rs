//! Pratt operator classification and binding-power tables.

use crate::expressions::SyntaxBinaryOperator;
use crate::grammar::kinds::SyntaxKind;

pub(super) fn is_postfix_operator(operator: &str) -> bool {
    matches!(operator, "(" | "[" | "." | "?")
}

pub(super) fn binary_binding_power(operator: &str) -> Option<(u8, u8, SyntaxKind)> {
    if operator == "=>" {
        return Some((0, 0, SyntaxKind::BinaryExpression));
    }
    let (power, kind) = match operator {
        "|>" => (1, SyntaxKind::PipeExpression),
        "||" => (3, SyntaxKind::BinaryExpression),
        "&&" => (5, SyntaxKind::BinaryExpression),
        "==" | "!=" => (7, SyntaxKind::BinaryExpression),
        "<" | "<=" | ">" | ">=" | "in" => (9, SyntaxKind::BinaryExpression),
        ".." | "..=" => (11, SyntaxKind::RangeExpression),
        "&" => (12, SyntaxKind::BinaryExpression),
        "+" | "-" => (13, SyntaxKind::BinaryExpression),
        "*" | "/" | "%" => (15, SyntaxKind::BinaryExpression),
        _ => return None,
    };
    Some((power, power + 1, kind))
}

pub(super) fn syntax_binary_operator(operator: &str) -> Option<SyntaxBinaryOperator> {
    Some(match operator {
        "=>" => SyntaxBinaryOperator::Implies,
        "||" => SyntaxBinaryOperator::Or,
        "&&" => SyntaxBinaryOperator::And,
        "in" => SyntaxBinaryOperator::In,
        "==" => SyntaxBinaryOperator::Equal,
        "!=" => SyntaxBinaryOperator::NotEqual,
        ">=" => SyntaxBinaryOperator::GreaterOrEqual,
        "<=" => SyntaxBinaryOperator::LessOrEqual,
        ">" => SyntaxBinaryOperator::Greater,
        "<" => SyntaxBinaryOperator::Less,
        "&" => SyntaxBinaryOperator::Merge,
        "+" => SyntaxBinaryOperator::Add,
        "-" => SyntaxBinaryOperator::Subtract,
        "*" => SyntaxBinaryOperator::Multiply,
        "/" => SyntaxBinaryOperator::Divide,
        "%" => SyntaxBinaryOperator::Remainder,
        _ => return None,
    })
}
