//! Stable textual labels used by runtime-plan lowering.

use arcweft_core::time::LogicalDuration;
use arcweft_lang_syntax::ast::{ids::EntityRefSyntax, pattern::Pattern};
use arcweft_lang_syntax::expr::{BinaryOp, CallArg, DurationUnit, Expr, Literal, UnaryOp};
use arcweft_lang_syntax::types::{TypeEffectRow, TypeRef};

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
            DurationUnit::Nanos => 1,
            DurationUnit::Micros => 1_000,
            DurationUnit::Millis => 1_000_000,
            DurationUnit::Seconds => 1_000_000_000,
            DurationUnit::Minutes => 60_000_000_000,
            DurationUnit::Hours => 3_600_000_000_000,
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
        Expr::Path(path) => path.as_label().to_owned(),
        Expr::ShortVariant(name) => format!(".{name}"),
        Expr::EntityRef(entity) => format!("@{}", entity_ref_label(entity)),
        Expr::Literal(literal) => literal_label(literal),
        Expr::Call(call) => format!(
            "{}({})",
            expr_label(call.callee()),
            call.args()
                .iter()
                .map(call_arg_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Select(select) => format!("{}.{}", expr_label(select.target()), select.member()),
        Expr::Index { target, index } => format!("{}[{}]", expr_label(target), expr_label(index)),
        Expr::Pipe { lhs, rhs } => format!("{} |> {}", expr_label(lhs), expr_label(rhs)),
        Expr::ArrayRepeat { value, len } => {
            format!("[{}; {}]", expr_label(value), expr_label(len))
        }
        Expr::Binary { lhs, op, rhs } => format!(
            "{} {} {}",
            expr_label(lhs),
            binary_op_label(*op),
            expr_label(rhs)
        ),
        Expr::Unary { op, expr } => format!("{}{}", unary_op_label(*op), expr_label(expr)),
        Expr::NumericBracketSeq(seq) => {
            let values = seq
                .literals()
                .iter()
                .map(arcweft_lang_syntax::expr::IntLiteral::raw)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        other => format!("{other:?}"),
    }
}

const fn binary_op_label(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Implies => "=>",
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
        BinaryOp::In => "in",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Gte => ">=",
        BinaryOp::Lte => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Lt => "<",
        BinaryOp::Merge => "&",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
    }
}

const fn unary_op_label(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
        UnaryOp::Neg => "-",
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
        Literal::Int(literal) => literal.raw().to_owned(),
        Literal::Bool(value) => value.to_string(),
        Literal::Duration { amount, unit } => format!("{amount}{}", unit.as_str()),
    }
}

pub(crate) fn type_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.canonical_string(),
        TypeRef::Tuple(items) => format!(
            "({})",
            items.iter().map(type_label).collect::<Vec<_>>().join(", ")
        ),
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            let params = if params.len() == 1 {
                type_label(&params[0])
            } else {
                format!(
                    "({})",
                    params.iter().map(type_label).collect::<Vec<_>>().join(", ")
                )
            };
            let label = format!("{params} -> {}", type_label(return_type));
            type_effect_row_label(effects.as_ref()).map_or(label.clone(), |effects| {
                format!("{label} effects {effects}")
            })
        }
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(type_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter().map(type_label).collect::<Vec<_>>().join(", ")
        ),
        TypeRef::TraitBound(bound) => {
            let mut args = bound.args().iter().map(type_label).collect::<Vec<_>>();
            args.extend(
                bound
                    .associated()
                    .iter()
                    .map(|binding| format!("{} = {}", binding.name(), type_label(binding.value()))),
            );
            format!("{}<{}>", bound.path(), args.join(", "))
        }
        TypeRef::Projection { subject, assoc } => {
            format!("{}::{}", type_label(subject), assoc.as_str())
        }
        TypeRef::Reference(reference) => {
            let lifetime = reference
                .region()
                .name()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!(
                "&{lifetime}{}{}",
                reference.kind().source_qualifier(),
                type_label(reference.referent())
            )
        }
        TypeRef::Slice(inner) => format!("[{}]", type_label(inner)),
        TypeRef::Recovery(_) => "<recovery>".to_owned(),
    }
}

fn type_effect_row_label(effects: Option<&TypeEffectRow>) -> Option<String> {
    effects.map(|effects| {
        if effects.effects().is_empty() {
            "{ }".to_owned()
        } else {
            format!("{{ {} }}", effects.effects().join(", "))
        }
    })
}

fn decimal_to_nanos(amount: &str, unit_nanos: u64) -> Option<u64> {
    let cleaned = amount.replace('_', "");
    let (whole, frac) = cleaned.split_once('.').unwrap_or((cleaned.as_str(), ""));
    let whole_nanos = whole.parse::<u64>().ok()?.checked_mul(unit_nanos)?;
    if frac.is_empty() {
        return Some(whole_nanos);
    }
    let scale = 10_u64.checked_pow(u32::try_from(frac.len()).ok()?)?;
    let frac_nanos = frac.parse::<u64>().ok()?.checked_mul(unit_nanos)? / scale;
    whole_nanos.checked_add(frac_nanos)
}
