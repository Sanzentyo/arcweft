use arcweft_lang_hir::{
    expr::HirExprKind,
    item::{HirFunctionBody, HirItemKind},
    leaf::{HirPathSegment, HirPathValue},
    module::HirModule,
    project::HirProject,
    source_index::{
        HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole,
    },
    stmt::HirStmtKind,
};
use arcweft_source::{SourceDocument, SourceRange};

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
    project: &HirProject,
    document: &SourceDocument,
    controller_name: &str,
    cell_source: SourceRange,
) -> Result<Vec<ReplBindingRecord>, String> {
    let module = project
        .view()
        .modules()
        .find_map(|(_, module)| {
            (module.provenance().source_identity() == document.identity()).then_some(module)
        })
        .ok_or_else(|| "compiled REPL project omitted its accepted synthetic source".to_owned())?;
    let function = module
        .source_ordered_items()
        .iter()
        .filter_map(|&id| module.resolve_item(id).ok())
        .find_map(|item| match item.kind() {
            HirItemKind::Function(function)
                if function
                    .name()
                    .resolved()
                    .is_some_and(|name| name.as_str() == controller_name) =>
            {
                Some(function)
            }
            _ => None,
        })
        .ok_or_else(|| "compiled REPL project omitted its synthetic controller".to_owned())?;
    let HirFunctionBody::Block { statements, .. } = function.body() else {
        return Ok(Vec::new());
    };

    let mut bindings = Vec::new();
    for &statement_id in statements {
        let statement_span = source_span(
            module,
            document,
            HirSourceQuery::Stmt {
                owner: statement_id,
                role: HirStmtSourceRole::Whole,
            },
        )?;
        if statement_span.start() < cell_source.start() || statement_span.end() > cell_source.end()
        {
            continue;
        }
        let statement = module
            .resolve_stmt(statement_id)
            .map_err(|error| error.to_string())?;
        let HirStmtKind::Let {
            initializer,
            locals,
            ..
        } = statement.kind()
        else {
            continue;
        };
        let [local_id] = locals.as_ref() else {
            continue;
        };
        let Some(snapshot_kind) = snapshot_expr_kind(module, *initializer)? else {
            continue;
        };
        let local = module
            .resolve_local(*local_id)
            .map_err(|error| error.to_string())?;
        if local.is_poisoned() {
            continue;
        }
        let expression_span = source_span(
            module,
            document,
            HirSourceQuery::Expr {
                owner: *initializer,
                role: HirExprSourceRole::Whole,
            },
        )?;
        let source = document
            .text()
            .get(expression_span.as_range())
            .ok_or_else(|| "accepted HIR expression source escaped its document".to_owned())?
            .trim()
            .to_owned();
        if source.is_empty() {
            continue;
        }
        bindings.push(ReplBindingRecord {
            name: local.name().as_str().to_owned(),
            cell_id,
            source,
            snapshot_kind,
            project_bound: snapshot_kind.project_bound(),
            status: ReplBindingStatus::Active,
            invalidated: None,
        });
    }
    bindings.sort_by(|left, right| left.name.cmp(&right.name));
    bindings.dedup_by(|left, right| left.name == right.name);
    Ok(bindings)
}

pub(crate) fn live_binding_prelude(bindings: &[ReplBindingRecord]) -> String {
    bindings
        .iter()
        .filter(|binding| binding.status == ReplBindingStatus::Active)
        .map(|binding| format!("let {} = {}", binding.name, binding.source))
        .collect::<Vec<_>>()
        .join("\n")
}

fn source_span(
    module: &HirModule,
    document: &SourceDocument,
    query: HirSourceQuery,
) -> Result<SourceRange, String> {
    let lookup = module
        .source_site(document.identity(), query)
        .map_err(|error| error.to_string())?;
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        return Err("REPL binding owner has no authored source span".to_owned());
    };
    span.validate_for(document)
        .map_err(|error| error.to_string())?;
    Ok(span.range())
}

fn snapshot_expr_kind(
    module: &HirModule,
    expression_id: arcweft_lang_hir::identity::ExprId,
) -> Result<Option<ReplBindingSnapshotKind>, String> {
    let expression = module
        .resolve_expr(expression_id)
        .map_err(|error| error.to_string())?;
    let kind = match expression.kind() {
        HirExprKind::Literal(_) | HirExprKind::NumericBracketSequence(_) => {
            Some(ReplBindingSnapshotKind::Literal)
        }
        HirExprKind::EntityReference(_) => Some(ReplBindingSnapshotKind::ProjectRef),
        HirExprKind::BracketSequence(sequence) => {
            let members = sequence
                .elements()
                .iter()
                .map(|&member| snapshot_expr_kind(module, member))
                .collect::<Result<Vec<_>, _>>()?;
            if members
                .iter()
                .all(|member| *member == Some(ReplBindingSnapshotKind::Literal))
            {
                Some(ReplBindingSnapshotKind::Literal)
            } else if members.iter().all(Option::is_some) {
                Some(ReplBindingSnapshotKind::ProjectRef)
            } else {
                None
            }
        }
        HirExprKind::Try(expression) => snapshot_expr_kind(module, expression.operand())?,
        HirExprKind::Await(expression) => snapshot_expr_kind(module, expression.operand())?,
        HirExprKind::Call(call) if call_arguments_are_snapshots(module, call.arguments())? => {
            call_snapshot_kind(module, call.callee().value_expression())?
        }
        _ => None,
    };
    Ok(kind)
}

fn call_arguments_are_snapshots(
    module: &HirModule,
    arguments: &[arcweft_lang_hir::expr::HirCallArgument],
) -> Result<bool, String> {
    for argument in arguments {
        if snapshot_expr_kind(module, argument.value())?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn call_snapshot_kind(
    module: &HirModule,
    callee: Option<arcweft_lang_hir::identity::ExprId>,
) -> Result<Option<ReplBindingSnapshotKind>, String> {
    let Some(callee) = callee else {
        return Ok(None);
    };
    let expression = module
        .resolve_expr(callee)
        .map_err(|error| error.to_string())?;
    match expression.kind() {
        HirExprKind::Path(path) => Ok(match path_name(path) {
            Some("observe") => Some(ReplBindingSnapshotKind::Observation),
            Some("read_resource") => Some(ReplBindingSnapshotKind::Resource),
            _ => None,
        }),
        HirExprKind::Select(select) => {
            let target = module
                .resolve_expr(select.target())
                .map_err(|error| error.to_string())?;
            let member = match select.member() {
                arcweft_lang_hir::expr::HirSelectedMember::Name(name) => Some(name.as_str()),
                arcweft_lang_hir::expr::HirSelectedMember::Missing => None,
            };
            Ok(match (target.kind(), member) {
                (HirExprKind::Path(path), Some("query")) if path_name(path) == Some("rag") => {
                    Some(ReplBindingSnapshotKind::RagContext)
                }
                _ => None,
            })
        }
        _ => Ok(None),
    }
}

fn path_name(path: &HirPathValue) -> Option<&str> {
    let path = path.as_resolved()?;
    let [segment] = path.segments() else {
        return None;
    };
    Some(match segment {
        HirPathSegment::Identifier(name) => name.as_str(),
        HirPathSegment::ProjectSymbol(name) => name.as_str(),
    })
}
