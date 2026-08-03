//! Direct closure-capture discovery over typed paths and lexical locals.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::AttachedCandidatePathProjection;
use arcweft_lang_syntax::attachment::source_file::AttachedPath;
use arcweft_source::SourceSpan;

use crate::expr::HirExprKind;
use crate::identity::{
    CaptureId, ExprId, HirLimit, LocalId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::leaf::{HirName, HirPath, HirPathRoot, HirPathSegment};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{CaptureAccess, HirCapture};
use crate::source_index::{HirInsertionPoint, HirSourceSite};

use super::StagedHirModuleTransaction;

pub(super) struct ClosureCaptureFrame {
    closure: ExprId,
    scope: ScopeId,
    pending: BTreeMap<LocalId, PendingCapture>,
}

struct PendingCapture {
    access: CaptureAccess,
    first_use: SourceSpan,
}

impl ClosureCaptureFrame {
    fn new(closure: ExprId, scope: ScopeId) -> Self {
        Self {
            closure,
            scope,
            pending: BTreeMap::new(),
        }
    }
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn begin_closure_captures(
        &mut self,
        closure: ExprId,
        scope: ScopeId,
    ) -> Result<(), HirLowerFailure> {
        if closure.module() != scope.module()
            || self
                .closure_capture_frames
                .iter()
                .any(|frame| frame.closure == closure)
        {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        self.closure_capture_frames
            .push(ClosureCaptureFrame::new(closure, scope));
        Ok(())
    }

    pub(super) fn finish_closure_captures(
        &mut self,
        closure: ExprId,
    ) -> Result<Box<[CaptureId]>, HirLowerFailure> {
        let frame = self
            .closure_capture_frames
            .pop()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        if frame.closure != closure {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        super::require_limit(HirLimit::SyntheticDescendantsPerOwner, frame.pending.len())?;

        let mut pending = frame.pending.into_iter().collect::<Vec<_>>();
        pending.sort_by_key(|(local, capture)| (capture.first_use.range().start(), *local));

        let mut captures = Vec::with_capacity(pending.len());
        for (ordinal, (local, pending)) in pending.into_iter().enumerate() {
            let ordinal =
                u32::try_from(ordinal).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let key = SyntheticKey::try_new(
                SyntheticOwner::Expr(closure),
                SyntheticRole::ClosureCapture,
                ordinal,
            )
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
            let insertion = HirInsertionPoint::try_new(
                self.request.source().document(),
                pending.first_use.range().start(),
            )
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
            let payload = HirCapture::try_new(closure, local, pending.access, pending.first_use)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            captures.push(self.arenas.captures().allocate_synthetic(
                &mut self.slots,
                key,
                HirSourceSite::Insertion(insertion),
                payload,
            )?);
        }
        Ok(captures.into_boxed_slice())
    }

    pub(super) fn record_attached_path_capture(
        &mut self,
        scope: ScopeId,
        source: &AttachedPath,
        path: &HirPath,
    ) -> Result<(), HirLowerFailure> {
        let [segment] = source.segments() else {
            return Ok(());
        };
        self.record_path_capture(scope, path, segment.source_span(), CaptureAccess::Read)
    }

    pub(super) fn record_candidate_path_capture(
        &mut self,
        scope: ScopeId,
        source: AttachedCandidatePathProjection<'_>,
        path: &HirPath,
    ) -> Result<(), HirLowerFailure> {
        let mut segments = source.segments();
        if segments.len() != 1 {
            return Ok(());
        }
        let segment = segments
            .next()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        self.record_path_capture(scope, path, segment.source_span(), CaptureAccess::Read)
    }

    pub(super) fn record_local_capture(
        &mut self,
        scope: ScopeId,
        local: LocalId,
        first_use: SourceSpan,
        access: CaptureAccess,
    ) -> Result<(), HirLowerFailure> {
        if self.closure_capture_frames.is_empty() {
            return Ok(());
        }
        if first_use.source() != self.request.source().document().identity()
            || first_use.range().start() >= first_use.range().end()
        {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let (local_scope, local_name, poisoned) = {
            let local = self.arenas.locals.resolve_staged(&self.slots, local)?;
            (local.scope(), local.name().clone(), local.is_poisoned())
        };
        if poisoned
            || self.visible_local(scope, &local_name, first_use.range().start())? != Some(local)
        {
            return Err(HirInvariantFailure::InvalidLocalTimeline.into());
        }
        let mut frame_indices = Vec::new();
        for index in 0..self.closure_capture_frames.len() {
            let closure_scope = self.closure_capture_frames[index].scope;
            if !self.scope_descends_from(scope, closure_scope)? {
                return Err(HirInvariantFailure::InvalidScopeParent.into());
            }
            if !self.scope_descends_from(local_scope, closure_scope)? {
                frame_indices.push(index);
            }
        }
        for index in frame_indices {
            self.record_pending_capture(index, local, first_use.clone(), access)?;
        }
        Ok(())
    }

    pub(super) fn upgrade_direct_reassignment_capture(
        &mut self,
        expression: ExprId,
    ) -> Result<(), HirLowerFailure> {
        if self.closure_capture_frames.is_empty() {
            return Ok(());
        }
        let (scope, path) = {
            let expression = self
                .arenas
                .expressions()
                .resolve_staged(&self.slots, expression)?;
            let HirExprKind::Path(crate::leaf::HirPathValue::Resolved(path)) = expression.kind()
            else {
                return Ok(());
            };
            (expression.scope(), path.clone())
        };
        let Some(name) = local_reference_name(&path) else {
            return Ok(());
        };
        let source = match self.slots.resolve_staged(expression)?.source_site() {
            HirSourceSite::Span(source) => source.clone(),
            HirSourceSite::Insertion(_) => {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        };
        let Some(local) = self.visible_local(scope, name, source.range().start())? else {
            return Ok(());
        };
        self.record_local_capture(scope, local, source, CaptureAccess::Reassign)
    }

    fn record_path_capture(
        &mut self,
        scope: ScopeId,
        path: &HirPath,
        first_use: SourceSpan,
        access: CaptureAccess,
    ) -> Result<(), HirLowerFailure> {
        let Some(name) = local_reference_name(path) else {
            return Ok(());
        };
        let Some(local) = self.visible_local(scope, name, first_use.range().start())? else {
            return Ok(());
        };
        self.record_local_capture(scope, local, first_use, access)
    }

    fn record_pending_capture(
        &mut self,
        frame_index: usize,
        local: LocalId,
        first_use: SourceSpan,
        access: CaptureAccess,
    ) -> Result<(), HirLowerFailure> {
        let frame = &mut self.closure_capture_frames[frame_index];
        if let Some(pending) = frame.pending.get_mut(&local) {
            if pending.first_use.source() != first_use.source() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let retained_order = (
                pending.first_use.range().start(),
                pending.first_use.range().end(),
            );
            let candidate_order = (first_use.range().start(), first_use.range().end());
            if candidate_order < retained_order {
                pending.first_use = first_use;
            }
            if access == CaptureAccess::Reassign {
                pending.access = CaptureAccess::Reassign;
            }
            return Ok(());
        }
        frame
            .pending
            .insert(local, PendingCapture { access, first_use });
        Ok(())
    }

    fn scope_descends_from(
        &self,
        scope: ScopeId,
        ancestor: ScopeId,
    ) -> Result<bool, HirLowerFailure> {
        let mut current = Some(scope);
        let mut visited = BTreeSet::new();
        while let Some(scope) = current {
            if !visited.insert(scope) {
                return Err(HirInvariantFailure::InvalidScopeParent.into());
            }
            if scope == ancestor {
                return Ok(true);
            }
            current = self
                .arenas
                .scopes
                .resolve_staged(&self.slots, scope)?
                .parent();
        }
        Ok(false)
    }
}

fn local_reference_name(path: &HirPath) -> Option<&HirName> {
    if path.root() != HirPathRoot::ImplicitCrate {
        return None;
    }
    let [HirPathSegment::Identifier(name)] = path.segments() else {
        return None;
    };
    Some(name)
}
