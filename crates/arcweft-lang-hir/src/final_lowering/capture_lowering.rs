//! Direct closure-capture discovery over typed paths and lexical locals.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::AttachedCandidatePathProjection;
use arcweft_lang_syntax::attachment::source_file::AttachedPath;
use arcweft_source::SourceSpan;

use crate::identity::{
    CaptureId, ExprId, HirLimit, LocalId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::leaf::{HirName, HirPath};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{CaptureAccess, HirCapture, HirCaptureUse, HirCaptureUseSite};
use crate::source_index::{HirInsertionPoint, HirSourceSite};

use super::StagedHirModuleTransaction;

pub(super) struct ClosureCaptureFrame {
    closure: ExprId,
    scope: ScopeId,
    pending: BTreeMap<HirCaptureUseSite, (LocalId, HirCaptureUse)>,
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
        let mut by_local = BTreeMap::<LocalId, Vec<HirCaptureUse>>::new();
        for (local, use_site) in frame.pending.into_values() {
            by_local.entry(local).or_default().push(use_site);
        }
        super::require_limit(HirLimit::SyntheticDescendantsPerOwner, by_local.len())?;
        let mut pending = by_local
            .into_iter()
            .map(|(local, mut uses)| {
                uses.sort_by_key(|use_site| {
                    (
                        use_site.source().range().start(),
                        use_site.source().range().end(),
                        use_site.site(),
                    )
                });
                HirCapture::try_new(closure, local, uses.into())
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)
            })
            .collect::<Result<Vec<_>, _>>()?;
        pending.sort_by_key(|capture| (capture.first_use().range().start(), capture.local()));

        let mut captures = Vec::with_capacity(pending.len());
        for (ordinal, payload) in pending.into_iter().enumerate() {
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
                payload.first_use().range().start(),
            )
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
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
        owner: ExprId,
        scope: ScopeId,
        source: &AttachedPath,
        path: &HirPath,
    ) -> Result<(), HirLowerFailure> {
        let [segment] = source.segments() else {
            return Ok(());
        };
        self.record_path_capture(owner, scope, path, segment.source_span())
    }

    pub(super) fn record_candidate_path_capture(
        &mut self,
        owner: ExprId,
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
        self.record_path_capture(owner, scope, path, segment.source_span())
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "each crossed closure retains the exact source evidence for this lexical use"
    )]
    pub(super) fn record_local_capture(
        &mut self,
        site: HirCaptureUseSite,
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
            self.record_pending_capture(
                index,
                local,
                HirCaptureUse::new(site, access, first_use.clone()),
            )?;
        }
        Ok(())
    }

    pub(super) fn upgrade_direct_reassignment_capture(&mut self, expression: ExprId) {
        for frame in &mut self.closure_capture_frames {
            if let Some((_, use_site)) = frame.pending.get_mut(&HirCaptureUseSite::Path(expression))
            {
                use_site.require_access(CaptureAccess::Reassign);
            }
        }
    }

    fn record_path_capture(
        &mut self,
        owner: ExprId,
        scope: ScopeId,
        path: &HirPath,
        first_use: SourceSpan,
    ) -> Result<(), HirLowerFailure> {
        let Some(name) = path.lexical_name() else {
            return Ok(());
        };
        let Ok(name) = HirName::try_new(name.into()) else {
            return Ok(());
        };
        let Some(local) = self.visible_local(scope, &name, first_use.range().start())? else {
            return Ok(());
        };
        self.record_local_capture(
            HirCaptureUseSite::Path(owner),
            scope,
            local,
            first_use,
            CaptureAccess::Read,
        )
    }

    fn record_pending_capture(
        &mut self,
        frame_index: usize,
        local: LocalId,
        use_site: HirCaptureUse,
    ) -> Result<(), HirLowerFailure> {
        let frame = &mut self.closure_capture_frames[frame_index];
        if let Some((retained_local, pending)) = frame.pending.get_mut(&use_site.site()) {
            if *retained_local != local || pending.source() != use_site.source() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            pending.require_access(use_site.access());
            return Ok(());
        }
        frame.pending.insert(use_site.site(), (local, use_site));
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
