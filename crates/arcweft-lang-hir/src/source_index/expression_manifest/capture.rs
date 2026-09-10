//! Complete lexical-use validation during the attached expression traversal.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedCandidateNode, AttachedCandidatePathExpression, AttachedExpressionNode,
};
use arcweft_lang_syntax::expressions::{ExpressionComponentRole, ExpressionRecordFieldPart};
use arcweft_source::SourceSpan;

use crate::arena::ArenaSnapshot;
use crate::expr::{HirExpr, HirExprKind};
use crate::identity::{CaptureId, ExprId, LocalId, ScopeId, StmtId};
use crate::leaf::HirPathValue;
use crate::module::HirLocalResolver;
use crate::scope::{
    CaptureAccess, HirCapture, HirCaptureUse, HirCaptureUseSite, HirLocal, HirScope, HirScopeKind,
    HirScopeOwner, LocalLookup,
};
use crate::slot::SlotSnapshot;
use crate::stmt::{HirStmt, HirStmtKind};

/// Borrowed indexes and an observation ledger disappear at source freeze. The
/// immutable capture records remain the sole published lexical-use inventory.
pub(super) struct CaptureValidation<'a> {
    slots: &'a SlotSnapshot,
    scopes: &'a ArenaSnapshot<HirScope, ScopeId>,
    locals: &'a ArenaSnapshot<HirLocal, LocalId>,
    resolver: &'a HirLocalResolver<'a>,
    uses: BTreeMap<(ExprId, LocalId, HirCaptureUseSite), &'a HirCaptureUse>,
    assignments: BTreeSet<ExprId>,
    closure_scopes: BTreeSet<ScopeId>,
    observed: BTreeSet<(ExprId, LocalId, HirCaptureUseSite)>,
    expected_uses: usize,
}

impl<'a> CaptureValidation<'a> {
    pub(super) fn new(
        slots: &'a SlotSnapshot,
        scopes: &'a ArenaSnapshot<HirScope, ScopeId>,
        locals: &'a ArenaSnapshot<HirLocal, LocalId>,
        statements: &'a ArenaSnapshot<HirStmt, StmtId>,
        captures: &'a ArenaSnapshot<HirCapture, CaptureId>,
        resolver: &'a HirLocalResolver<'a>,
    ) -> Option<Self> {
        let mut uses = BTreeMap::new();
        let mut expected_uses = 0_usize;
        for (_, capture) in captures.try_iter_prepared(slots).ok()? {
            expected_uses = expected_uses.checked_add(capture.uses().len())?;
            for use_site in capture.uses().iter() {
                if uses
                    .insert(
                        (capture.closure(), capture.local(), use_site.site()),
                        use_site,
                    )
                    .is_some()
                {
                    return None;
                }
            }
        }
        let assignments = statements
            .try_iter_prepared(slots)
            .ok()?
            .filter_map(|(_, statement)| {
                if let HirStmtKind::Assign { target, .. } = statement.kind() {
                    Some(*target)
                } else {
                    None
                }
            })
            .collect();
        let mut closure_scopes = BTreeSet::new();
        let mut pending = scopes
            .try_iter_prepared(slots)
            .ok()?
            .filter_map(|(owner, scope)| (scope.kind() == HirScopeKind::Closure).then_some(owner))
            .collect::<Vec<_>>();
        while let Some(owner) = pending.pop() {
            if closure_scopes.insert(owner) {
                pending.extend(scopes.resolve_prepared(slots, owner).ok()?.children());
            }
        }
        Some(Self {
            slots,
            scopes,
            locals,
            resolver,
            uses,
            assignments,
            closure_scopes,
            observed: BTreeSet::new(),
            expected_uses,
        })
    }

    pub(super) fn complete(&self) -> bool {
        self.observed.len() == self.expected_uses
    }

    pub(super) fn attached(
        &mut self,
        owner: ExprId,
        expression: &HirExpr,
        attached: &AttachedExpressionNode,
    ) -> Option<()> {
        let path_source = attached.path().and_then(|path| {
            if let [segment] = path.segments() {
                Some(segment.source_span())
            } else {
                None
            }
        });
        self.expression(owner, expression, path_source, |field| {
            attached.component(ExpressionComponentRole::RecordField {
                field,
                part: ExpressionRecordFieldPart::Name,
            })
        })
    }

    pub(super) fn candidate(
        &mut self,
        owner: ExprId,
        expression: &HirExpr,
        node: AttachedCandidateNode<'_>,
    ) -> Option<()> {
        let path_source = match node.path_expression_view() {
            Some(AttachedCandidatePathExpression::Value(path)) => {
                let mut segments = path.segments();
                if segments.len() == 1 {
                    segments.next().map(
                        arcweft_lang_syntax::attachment::AttachedCandidatePathSegment::source_span,
                    )
                } else {
                    None
                }
            }
            _ => None,
        };
        self.expression(owner, expression, path_source, |field| {
            node.expression_components()?
                .find(|component| {
                    component.role()
                        == ExpressionComponentRole::RecordField {
                            field,
                            part: ExpressionRecordFieldPart::Name,
                        }
                })
                .map(|component| component.source_span().clone())
        })
    }

    fn expression(
        &mut self,
        owner: ExprId,
        expression: &HirExpr,
        path_source: Option<SourceSpan>,
        mut field_source: impl FnMut(u32) -> Option<SourceSpan>,
    ) -> Option<()> {
        let fields = match expression.kind() {
            HirExprKind::Path(HirPathValue::Resolved(path)) => {
                if let (Some(name), Some(source)) = (path.lexical_name(), path_source) {
                    self.reference(
                        HirCaptureUseSite::Path(owner),
                        expression.scope(),
                        name,
                        &source,
                    )?;
                }
                return Some(());
            }
            HirExprKind::Record(record) => record.fields(),
            HirExprKind::RecordLiteral(record) => record.fields(),
            _ => return Some(()),
        };
        for (field, value) in fields.iter().enumerate() {
            if value.local().is_some() {
                let field = u32::try_from(field).ok()?;
                let source = field_source(field)?;
                self.reference(
                    HirCaptureUseSite::RecordShorthand { owner, field },
                    expression.scope(),
                    value.name()?.as_str(),
                    &source,
                )?;
            }
        }
        Some(())
    }

    fn reference(
        &mut self,
        site: HirCaptureUseSite,
        scope: ScopeId,
        name: &str,
        source: &SourceSpan,
    ) -> Option<()> {
        if !self.closure_scopes.contains(&scope) {
            return Some(());
        }
        let LocalLookup::Found(local) =
            self.resolver.lookup(scope, name, source.range().start())?
        else {
            return Some(());
        };
        let local_scope = self
            .locals
            .resolve_prepared(self.slots, local)
            .ok()?
            .scope();
        let access = if matches!(site, HirCaptureUseSite::Path(owner) if self.assignments.contains(&owner))
        {
            CaptureAccess::Reassign
        } else {
            CaptureAccess::Read
        };
        let mut current = scope;
        let mut visited = BTreeSet::new();
        while current != local_scope {
            if !visited.insert(current) {
                return None;
            }
            let scope = self.scopes.resolve_prepared(self.slots, current).ok()?;
            if scope.kind() == HirScopeKind::Closure {
                let HirScopeOwner::Expr(closure) = scope.owner() else {
                    return None;
                };
                let actual = self.uses.get(&(*closure, local, site))?;
                if actual.source() != source
                    || actual.access() != access
                    || !self.observed.insert((*closure, local, site))
                {
                    return None;
                }
            }
            current = scope.parent()?;
        }
        Some(())
    }
}
