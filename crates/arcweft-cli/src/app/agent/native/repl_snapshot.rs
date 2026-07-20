use super::repl::AgentReplSerializedBinding;
use arcweft_lang_syntax::{
    ast::{flow::Stmt, ids::EntityRefSyntax, pattern::Pattern},
    expr::{CallArg, Expr, Literal},
    parser::{ParsedFragment, ParsedFragmentKind},
};
use std::collections::BTreeMap;

pub(super) fn agent_repl_serialized_bindings(
    fragment: &ParsedFragment,
) -> BTreeMap<String, AgentReplSerializedBinding> {
    let Some(ParsedFragmentKind::Statements(statements)) = fragment.kind() else {
        return BTreeMap::new();
    };
    statements
        .iter()
        .filter_map(agent_repl_serialized_stmt_binding)
        .collect()
}

fn agent_repl_serialized_stmt_binding(
    statement: &Stmt,
) -> Option<(String, AgentReplSerializedBinding)> {
    let Stmt::Let {
        pattern,
        expr,
        expr_source,
        ..
    } = statement
    else {
        return None;
    };
    let name = agent_repl_single_binding_name(pattern)?;
    let binding = agent_repl_serialized_expr_binding(expr, expr_source.as_deref())?;
    Some((name, binding))
}

fn agent_repl_single_binding_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn agent_repl_serialized_expr_source(expr: &Expr) -> Option<String> {
    agent_repl_serialized_expr_source_and_kind(expr).map(|(source, _)| source)
}

fn agent_repl_serialized_expr_binding(
    expr: &Expr,
    expr_source: Option<&str>,
) -> Option<AgentReplSerializedBinding> {
    agent_repl_serialized_expr_source_and_kind(expr)
        .map(|(source, snapshot_kind)| AgentReplSerializedBinding {
            source,
            snapshot_kind: snapshot_kind.to_owned(),
        })
        .or_else(|| {
            let snapshot_kind = agent_repl_snapshot_expr_kind(expr)?;
            let source = expr_source?.trim();
            (!source.is_empty()).then(|| AgentReplSerializedBinding {
                source: source.to_owned(),
                snapshot_kind: snapshot_kind.to_owned(),
            })
        })
}

fn agent_repl_serialized_expr_source_and_kind(expr: &Expr) -> Option<(String, &'static str)> {
    match expr {
        Expr::Literal(literal) => {
            agent_repl_serialized_literal_source(literal).map(|source| (source, "literal"))
        }
        Expr::EntityRef(entity) => {
            agent_repl_serialized_entity_ref_source(entity).map(|source| (source, "project_ref"))
        }
        Expr::NumericBracketSeq(seq) => Some((
            agent_repl_serialized_numeric_bracket_seq_source(seq),
            "literal",
        )),
        Expr::BracketSeq(items) => {
            let items = items
                .iter()
                .map(agent_repl_serialized_expr_source_and_kind)
                .collect::<Option<Vec<_>>>()?;
            let snapshot_kind = if items
                .iter()
                .all(|(_, snapshot_kind)| *snapshot_kind == "literal")
            {
                "literal"
            } else {
                "project_ref"
            };
            let source = items
                .into_iter()
                .map(|(source, _)| source)
                .collect::<Vec<_>>()
                .join(", ");
            Some((format!("[{source}]"), snapshot_kind))
        }
        _ => None,
    }
}

fn agent_repl_serialized_numeric_bracket_seq_source(
    seq: &arcweft_lang_syntax::expr::NumericBracketSeq,
) -> String {
    let values = seq
        .literals()
        .iter()
        .map(arcweft_lang_syntax::expr::IntLiteral::raw)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn agent_repl_snapshot_expr_kind(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::Try { expr } => agent_repl_snapshot_expr_kind(expr),
        Expr::Await(awaited) => agent_repl_snapshot_expr_kind(awaited.operand()),
        Expr::Call(call) if agent_repl_snapshot_call_args_are_self_contained(call.args()) => {
            agent_repl_call_snapshot_kind(call.callee())
        }
        _ => None,
    }
}

fn agent_repl_call_snapshot_kind(callee: &Expr) -> Option<&'static str> {
    match callee {
        Expr::Path(name) if name == "observe" => Some("observation"),
        Expr::Path(name) if name == "read_resource" => Some("resource"),
        Expr::Select(select) if select.member().as_str() == "query" => {
            agent_repl_method_snapshot_kind(select.target(), select.member().as_str())
        }
        _ => None,
    }
}

fn agent_repl_method_snapshot_kind(receiver: &Expr, method: &str) -> Option<&'static str> {
    match (receiver, method) {
        (Expr::Path(namespace), "query") if namespace == "rag" => Some("rag_context"),
        _ => None,
    }
}

fn agent_repl_snapshot_call_args_are_self_contained(args: &[CallArg]) -> bool {
    args.iter().all(|arg| match arg {
        CallArg::Positional(expr) => agent_repl_serialized_expr_source(expr).is_some(),
        CallArg::Named { value, .. } | CallArg::Spread { value } => {
            agent_repl_serialized_expr_source(value).is_some()
        }
    })
}

fn agent_repl_serialized_literal_source(literal: &Literal) -> Option<String> {
    match literal {
        Literal::String(value) => serde_json::to_string(value).ok(),
        Literal::Char { raw, .. }
        | Literal::Float { raw, .. }
        | Literal::UnitNumber { raw, .. } => Some(raw.clone()),
        Literal::Int(literal) => Some(literal.raw().to_owned()),
        Literal::Bool(value) => Some(value.to_string()),
        Literal::Duration { .. } => None,
    }
}

fn agent_repl_serialized_entity_ref_source(entity: &EntityRefSyntax) -> Option<String> {
    entity
        .as_absolute()
        .map(|entity| format!("@{}", entity.body()))
}
