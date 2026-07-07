use std::collections::BTreeMap;

use arcweft_lang_syntax::ast::{
    flow::Stmt,
    ids::EntityRefSyntax,
    pattern::{Pattern, VariantPatternPayload},
};
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal};
use arcweft_lang_syntax::parser::{ParsedFragment, ParsedFragmentKind};

use crate::cell::ReplCellId;
use crate::evidence::ReplGenerationId;

/// Snapshot family for a binding that may be reintroduced in later cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplBindingSnapshotKind {
    Literal,
    ProjectRef,
    Observation,
    Resource,
    RagContext,
}

/// Binding validity in the current base project/generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplBindingStatus {
    Active,
    Invalidated,
}

/// Reason captured when a project-bound binding becomes invalid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplBindingInvalidation {
    pub reason: String,
    pub old_program_hash: String,
    pub new_program_hash: String,
    pub old_generation: ReplGenerationId,
    pub new_generation: ReplGenerationId,
}

/// Public binding evidence produced by committed cells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplBindingRecord {
    pub name: String,
    pub cell_id: ReplCellId,
    pub source: String,
    pub snapshot_kind: ReplBindingSnapshotKind,
    pub project_bound: bool,
    pub status: ReplBindingStatus,
    pub invalidated: Option<ReplBindingInvalidation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SerializedBinding {
    pub(crate) source: String,
    pub(crate) snapshot_kind: ReplBindingSnapshotKind,
}

impl ReplBindingSnapshotKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::ProjectRef => "project_ref",
            Self::Observation => "observation",
            Self::Resource => "resource",
            Self::RagContext => "rag_context",
        }
    }

    #[must_use]
    pub const fn project_bound(self) -> bool {
        !matches!(self, Self::Literal)
    }
}

impl ReplBindingRecord {
    pub(crate) fn invalidate(&mut self, invalidation: ReplBindingInvalidation) {
        self.status = ReplBindingStatus::Invalidated;
        self.invalidated = Some(invalidation);
    }
}

pub(crate) fn committed_bindings(
    cell_id: ReplCellId,
    fragment: &ParsedFragment,
) -> Vec<ReplBindingRecord> {
    let serialized = serialized_bindings(fragment);
    fragment_binding_names(fragment)
        .into_iter()
        .filter_map(|name| serialized.get(&name).map(|snapshot| (name, snapshot)))
        .map(|(name, snapshot)| ReplBindingRecord {
            name,
            cell_id,
            source: snapshot.source.clone(),
            snapshot_kind: snapshot.snapshot_kind,
            project_bound: snapshot.snapshot_kind.project_bound(),
            status: ReplBindingStatus::Active,
            invalidated: None,
        })
        .collect()
}

pub(crate) fn live_binding_prelude(bindings: &[ReplBindingRecord]) -> String {
    bindings
        .iter()
        .filter(|binding| binding.status == ReplBindingStatus::Active)
        .map(|binding| format!("let {} = {}", binding.name, binding.source))
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialized_bindings(fragment: &ParsedFragment) -> BTreeMap<String, SerializedBinding> {
    let Some(ParsedFragmentKind::Statements(statements)) = fragment.kind() else {
        return BTreeMap::new();
    };
    statements
        .iter()
        .filter_map(serialized_stmt_binding)
        .collect()
}

fn serialized_stmt_binding(statement: &Stmt) -> Option<(String, SerializedBinding)> {
    let Stmt::Let {
        pattern,
        expr,
        expr_source,
        ..
    } = statement
    else {
        return None;
    };
    let name = single_binding_name(pattern)?;
    let binding = serialized_expr_binding(expr, expr_source.as_deref())?;
    Some((name, binding))
}

fn single_binding_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn serialized_expr_source(expr: &Expr) -> Option<String> {
    serialized_expr_source_and_kind(expr).map(|(source, _)| source)
}

fn serialized_expr_binding(expr: &Expr, expr_source: Option<&str>) -> Option<SerializedBinding> {
    serialized_expr_source_and_kind(expr)
        .map(|(source, snapshot_kind)| SerializedBinding {
            source,
            snapshot_kind,
        })
        .or_else(|| {
            let snapshot_kind = snapshot_expr_kind(expr)?;
            let source = expr_source?.trim();
            (!source.is_empty()).then(|| SerializedBinding {
                source: source.to_owned(),
                snapshot_kind,
            })
        })
}

fn serialized_expr_source_and_kind(expr: &Expr) -> Option<(String, ReplBindingSnapshotKind)> {
    match expr {
        Expr::Literal(literal) => serialized_literal_source(literal)
            .map(|source| (source, ReplBindingSnapshotKind::Literal)),
        Expr::EntityRef(entity) => serialized_entity_ref_source(entity)
            .map(|source| (source, ReplBindingSnapshotKind::ProjectRef)),
        Expr::NumericBracketSeq(seq) => Some((
            serialized_numeric_bracket_seq_source(seq),
            ReplBindingSnapshotKind::Literal,
        )),
        Expr::BracketSeq(items) => {
            let items = items
                .iter()
                .map(serialized_expr_source_and_kind)
                .collect::<Option<Vec<_>>>()?;
            let snapshot_kind = if items
                .iter()
                .all(|(_, snapshot_kind)| *snapshot_kind == ReplBindingSnapshotKind::Literal)
            {
                ReplBindingSnapshotKind::Literal
            } else {
                ReplBindingSnapshotKind::ProjectRef
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

fn serialized_numeric_bracket_seq_source(
    seq: &arcweft_lang_syntax::expr::NumericBracketSeq,
) -> String {
    let suffix = seq.suffix().unwrap_or_default();
    let values = seq
        .values()
        .iter()
        .map(|value| format!("{value}{suffix}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn snapshot_expr_kind(expr: &Expr) -> Option<ReplBindingSnapshotKind> {
    match expr {
        Expr::Try { expr } | Expr::Await { expr, .. } => snapshot_expr_kind(expr),
        Expr::Call { callee, args } if snapshot_call_args_are_self_contained(args) => {
            call_snapshot_kind(callee.as_ref())
        }
        _ => None,
    }
}

fn call_snapshot_kind(callee: &Expr) -> Option<ReplBindingSnapshotKind> {
    match callee {
        Expr::Path(name) if name == "observe" => Some(ReplBindingSnapshotKind::Observation),
        Expr::Path(name) if name == "read_resource" => Some(ReplBindingSnapshotKind::Resource),
        Expr::Select(select) if select.member().as_str() == "query" => {
            method_snapshot_kind(select.target(), select.member().as_str())
        }
        _ => None,
    }
}

fn method_snapshot_kind(receiver: &Expr, method: &str) -> Option<ReplBindingSnapshotKind> {
    match (receiver, method) {
        (Expr::Path(namespace), "query") if namespace == "rag" => {
            Some(ReplBindingSnapshotKind::RagContext)
        }
        _ => None,
    }
}

fn snapshot_call_args_are_self_contained(args: &[CallArg]) -> bool {
    args.iter().all(|arg| match arg {
        CallArg::Positional(expr) => serialized_expr_source(expr).is_some(),
        CallArg::Named { value, .. } | CallArg::Spread { value } => {
            serialized_expr_source(value).is_some()
        }
    })
}

fn serialized_literal_source(literal: &Literal) -> Option<String> {
    match literal {
        Literal::String(value) => serde_json::to_string(value).ok(),
        Literal::Char { raw, .. } => Some(raw.clone()),
        Literal::Int { raw, .. } | Literal::Float { raw, .. } | Literal::UnitNumber { raw, .. } => {
            Some(raw.clone())
        }
        Literal::Bool(value) => Some(value.to_string()),
        Literal::Duration { .. } => None,
    }
}

fn serialized_entity_ref_source(entity: &EntityRefSyntax) -> Option<String> {
    entity
        .as_absolute()
        .map(|entity| format!("@{}", entity.body()))
}

fn fragment_binding_names(fragment: &ParsedFragment) -> Vec<String> {
    let Some(ParsedFragmentKind::Statements(statements)) = fragment.kind() else {
        return Vec::new();
    };
    let mut names = statements
        .iter()
        .flat_map(stmt_binding_names)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn stmt_binding_names(statement: &Stmt) -> Vec<String> {
    match statement {
        Stmt::Let { pattern, .. }
        | Stmt::LetElse { pattern, .. }
        | Stmt::LetChoice { pattern, .. }
        | Stmt::LetScope { pattern, .. }
        | Stmt::LetLoop { pattern, .. }
        | Stmt::LetAwait { pattern, .. }
        | Stmt::LetActionReceive { pattern, .. } => pattern_binding_names(pattern),
        Stmt::WhileLet { pattern, .. } | Stmt::For { pattern, .. } => {
            pattern_binding_names(pattern)
        }
        Stmt::Return(_)
        | Stmt::Assign { .. }
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Thread(_)
        | Stmt::DeferBlock { .. }
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Expr(_)
        | Stmt::Wait(_)
        | Stmt::On { .. }
        | Stmt::UnsafeLifetime { .. }
        | Stmt::If { .. }
        | Stmt::Match { .. }
        | Stmt::Loop { .. }
        | Stmt::While { .. }
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => Vec::new(),
    }
}

fn pattern_binding_names(pattern: &Pattern) -> Vec<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            vec![name.clone()]
        }
        Pattern::Whole { name, pattern } => {
            let mut names = vec![name.clone()];
            names.extend(pattern_binding_names(pattern));
            names
        }
        Pattern::Tuple(items) => items.iter().flat_map(pattern_binding_names).collect(),
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| pattern_binding_names(field.pattern()))
            .collect(),
        Pattern::BracketSeq { items, rest } => {
            let mut names = items
                .iter()
                .flat_map(pattern_binding_names)
                .collect::<Vec<_>>();
            names.extend(rest.clone());
            names
        }
        Pattern::Variant { payload, .. } => payload
            .as_ref()
            .map(variant_payload_binding_names)
            .unwrap_or_default(),
        Pattern::Literal(_) | Pattern::Entity(_) | Pattern::Discard | Pattern::Raw(_) => Vec::new(),
    }
}

fn variant_payload_binding_names(payload: &VariantPatternPayload) -> Vec<String> {
    match payload {
        VariantPatternPayload::Tuple(items) => {
            items.iter().flat_map(pattern_binding_names).collect()
        }
        VariantPatternPayload::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| pattern_binding_names(field.pattern()))
            .collect(),
    }
}
