//! Stable textual labels used by runtime-plan lowering.

use arcweft_core::time::LogicalDuration;
use arcweft_lang_hir::syntax::ast::{ids::EntityRefSyntax, pattern::Pattern};
use arcweft_lang_hir::syntax::expr::{CallArg, DurationUnit, Expr, Literal};
use arcweft_lang_hir::syntax::types::TypeRef;

pub(crate) fn named_arg_label(value: &str) -> Option<String> {
    value.split_once(" = ").map(|(name, _)| name.to_owned())
}

pub(crate) fn named_arg_value(value: &str) -> Option<String> {
    value.split_once(" = ").map(|(_, value)| value.to_owned())
}

pub(crate) fn pattern_label(pattern: &Pattern) -> String {
    format!("{pattern:?}")
}

pub(crate) fn duration_expr(expr: &Expr) -> Option<LogicalDuration> {
    let Expr::Literal(Literal::Duration { amount, unit }) = expr else {
        return None;
    };
    decimal_to_nanos(
        amount,
        match unit {
            DurationUnit::Millis => 1_000_000,
            DurationUnit::Seconds => 1_000_000_000,
        },
    )
    .map(LogicalDuration::from_nanos)
}

pub(crate) fn entity_ref_label(entity: &EntityRefSyntax) -> String {
    if let Some(absolute) = entity.as_absolute() {
        return absolute.body().to_owned();
    }
    if let Some(relative) = entity.family_relative_ref() {
        return format!("{}.{}", relative.family(), relative.relative().suffix());
    }
    entity.body().to_owned()
}

pub(crate) fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::LifetimePath { key, optional } => {
            format!("'{}{}", key.as_dotted(), if *optional { "?" } else { "" })
        }
        Expr::Path(path) => path.clone(),
        Expr::EntityRef(entity) => format!("@{}", entity_ref_label(entity)),
        Expr::Literal(literal) => literal_label(literal),
        Expr::Call { callee, args } => format!(
            "{}({})",
            expr_label(callee),
            args.iter()
                .map(call_arg_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => format!(
            "{}.{}({})",
            expr_label(receiver),
            method,
            args.iter()
                .map(call_arg_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Field { target, field } => format!("{}.{}", expr_label(target), field),
        Expr::Pipe { lhs, rhs } => format!("{} |> {}", expr_label(lhs), expr_label(rhs)),
        Expr::ArrayRepeat { value, len } => {
            format!("[{}; {}]", expr_label(value), expr_label(len))
        }
        Expr::NumericBracketSeq(seq) => {
            let suffix = seq.suffix().unwrap_or_default();
            let values = seq
                .values()
                .iter()
                .map(|value| format!("{value}{suffix}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        other => format!("{other:?}"),
    }
}

pub(crate) fn call_arg_label(arg: &CallArg) -> String {
    match arg {
        CallArg::Positional(value) => expr_label(value),
        CallArg::Named { name, value } => format!("{name} = {}", expr_label(value)),
        CallArg::Spread { value } => format!("{}...", expr_label(value)),
    }
}

pub(crate) fn literal_label(literal: &Literal) -> String {
    match literal {
        Literal::String(value) => format!("\"{value}\""),
        Literal::Char { raw, .. }
        | Literal::Float { raw, .. }
        | Literal::UnitNumber { raw, .. } => raw.clone(),
        Literal::Int { value, .. } => value.to_string(),
        Literal::Bool(value) => value.to_string(),
        Literal::Duration { amount, unit } => format!(
            "{amount}{}",
            match unit {
                DurationUnit::Millis => "ms",
                DurationUnit::Seconds => "s",
            }
        ),
    }
}

pub(crate) fn type_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.clone(),
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(type_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter().map(type_label).collect::<Vec<_>>().join(", ")
        ),
        TypeRef::Ref { lifetime, inner } => {
            let lifetime = lifetime
                .as_ref()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!("&{lifetime}{}", type_label(inner))
        }
        TypeRef::Slice(inner) => format!("[{}]", type_label(inner)),
    }
}

fn decimal_to_nanos(amount: &str, unit_nanos: u64) -> Option<u64> {
    let (whole, frac) = amount.split_once('.').unwrap_or((amount, ""));
    let whole_nanos = whole.parse::<u64>().ok()?.checked_mul(unit_nanos)?;
    if frac.is_empty() {
        return Some(whole_nanos);
    }
    let scale = 10_u64.checked_pow(u32::try_from(frac.len()).ok()?)?;
    let frac_nanos = frac.parse::<u64>().ok()?.checked_mul(unit_nanos)? / scale;
    whole_nanos.checked_add(frac_nanos)
}
