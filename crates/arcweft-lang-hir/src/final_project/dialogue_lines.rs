use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use crate::identity::ExprId;
use crate::line_identity::{
    DialogueLineCollisionSite, DialogueLineDiagnostic, DialogueLineIdOrigin,
    DialogueLineSourceOrder, DialogueTextKeyOrigin, HirDialogueLineCandidate,
    HirDialogueLineSourceOwner, HirDialogueNamedScope,
};
use crate::lowering::HirModuleKey;

use super::HirProjectModule;

const MAX_PROJECT_DIALOGUE_LINE_CANDIDATES: usize = 262_144;
const MAX_PROJECT_DIALOGUE_LINE_DIAGNOSTICS: usize = 1_024;
const MAX_PROJECT_DIALOGUE_LINE_WORK: u32 = 786_432;
const INVENTORY_FINGERPRINT_DOMAIN: &[u8] = b"arcweft.hir.dialogue-line-inventory.v1\0";

/// Stable index into one accepted dialogue-line inventory generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineIndex(u32);

/// Crate-private deterministic identity of one canonical accepted inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DialogueLineInventoryFingerprint([u8; 32]);

impl DialogueLineInventoryFingerprint {
    #[cfg(test)]
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl DialogueLineIndex {
    fn try_from_offset(offset: usize) -> Result<Self, DialogueLineProjectFatal> {
        let value = u32::try_from(offset).map_err(|_| DialogueLineProjectFatal::IndexOverflow)?;
        Ok(Self(value))
    }

    const fn offset(self) -> usize {
        self.0 as usize
    }
}

/// Complete revision-bound source evidence for one accepted dialogue line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedDialogueLineSource {
    module: HirModuleKey,
    application: ExprId,
    owner: HirDialogueLineSourceOwner,
    named_scopes: Arc<[HirDialogueNamedScope]>,
    source_order: DialogueLineSourceOrder,
    application_span: SourceSpan,
    id_coordinate_span: Option<SourceSpan>,
    text_key_coordinate_span: Option<SourceSpan>,
}

impl AcceptedDialogueLineSource {
    pub const fn module(&self) -> &HirModuleKey {
        &self.module
    }

    pub const fn application(&self) -> ExprId {
        self.application
    }

    pub const fn owner(&self) -> &HirDialogueLineSourceOwner {
        &self.owner
    }

    pub fn named_scopes(&self) -> &[HirDialogueNamedScope] {
        &self.named_scopes
    }

    pub const fn source_order(&self) -> DialogueLineSourceOrder {
        self.source_order
    }

    pub const fn application_span(&self) -> &SourceSpan {
        &self.application_span
    }

    pub const fn id_coordinate_span(&self) -> Option<&SourceSpan> {
        self.id_coordinate_span.as_ref()
    }

    pub const fn text_key_coordinate_span(&self) -> Option<&SourceSpan> {
        self.text_key_coordinate_span.as_ref()
    }
}

/// One project-accepted dialogue line and its localization identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedDialogueLine {
    id: DialogueLineId,
    text_key: DialogueTextKey,
    id_origin: DialogueLineIdOrigin,
    text_key_origin: DialogueTextKeyOrigin,
    source: AcceptedDialogueLineSource,
}

impl AcceptedDialogueLine {
    pub const fn id(&self) -> &DialogueLineId {
        &self.id
    }

    pub const fn text_key(&self) -> &DialogueTextKey {
        &self.text_key
    }

    pub const fn id_origin(&self) -> DialogueLineIdOrigin {
        self.id_origin
    }

    pub const fn text_key_origin(&self) -> DialogueTextKeyOrigin {
        self.text_key_origin
    }

    pub const fn source(&self) -> &AcceptedDialogueLineSource {
        &self.source
    }
}

/// Immutable dialogue-line facts published by exactly one accepted HIR project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedDialogueLineInventory {
    records: Arc<[AcceptedDialogueLine]>,
    by_id: BTreeMap<DialogueLineId, DialogueLineIndex>,
    by_expr: BTreeMap<ExprId, DialogueLineIndex>,
    source_order: Arc<[DialogueLineIndex]>,
    cache_fingerprint: DialogueLineInventoryFingerprint,
}

impl AcceptedDialogueLineInventory {
    pub(crate) fn empty() -> Self {
        let records = Arc::from([]);
        Self {
            cache_fingerprint: fingerprint_inventory(&records),
            records,
            by_id: BTreeMap::new(),
            by_expr: BTreeMap::new(),
            source_order: Arc::from([]),
        }
    }

    pub fn records(&self) -> &[AcceptedDialogueLine] {
        &self.records
    }

    pub fn get(&self, id: &DialogueLineId) -> Option<&AcceptedDialogueLine> {
        self.by_id
            .get(id)
            .map(|index| &self.records[index.offset()])
    }

    pub fn for_expr(&self, expr: ExprId) -> Option<&AcceptedDialogueLine> {
        self.by_expr
            .get(&expr)
            .map(|index| &self.records[index.offset()])
    }

    pub fn source_ordered(&self) -> impl ExactSizeIterator<Item = &AcceptedDialogueLine> {
        self.source_order
            .iter()
            .map(|index| &self.records[index.offset()])
    }

    #[cfg(test)]
    pub(crate) const fn cache_fingerprint(&self) -> DialogueLineInventoryFingerprint {
        self.cache_fingerprint
    }
}

/// Complete deterministic project collision rejection; no project was published.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("dialogue line acceptance rejected the HIR project")]
pub struct DialogueLineProjectRejection {
    diagnostics: Arc<[DialogueLineDiagnostic]>,
}

impl DialogueLineProjectRejection {
    pub fn diagnostics(&self) -> &[DialogueLineDiagnostic] {
        &self.diagnostics
    }
}

/// Fatal dialogue-line transaction failure that cannot claim complete diagnostics.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueLineProjectFatal {
    #[error("dialogue line project candidate count {observed} exceeds maximum {maximum}")]
    CandidateLimit { observed: usize, maximum: usize },
    #[error("dialogue line project diagnostic count {observed} exceeds maximum {maximum}")]
    DiagnosticLimit { observed: usize, maximum: usize },
    #[error("dialogue line project work {observed} exceeds maximum {maximum}")]
    WorkLimit { observed: u32, maximum: u32 },
    #[error("dialogue line candidate source does not match module source")]
    SourceIdentityMismatch {
        module: Box<HirModuleKey>,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error("dialogue line candidate source order is not canonical in module {module:?}")]
    InvalidSourceOrder { module: HirModuleKey },
    #[error("dialogue line candidate expression does not belong to module {module:?}")]
    ForeignExpression {
        module: Box<HirModuleKey>,
        expression: ExprId,
    },
    #[error("dialogue line candidate expression occurs more than once: {expression:?}")]
    DuplicateExpression { expression: ExprId },
    #[error("dialogue line inventory index does not fit its fixed index type")]
    IndexOverflow,
}

pub(crate) fn accept_dialogue_lines<'module>(
    modules: impl Iterator<Item = &'module HirProjectModule>,
) -> Result<AcceptedDialogueLineInventory, super::HirProjectBuildError> {
    let mut transaction = DialogueLineAcceptanceTransaction::new();
    for module in modules {
        transaction.accept_module(module)?;
    }
    transaction.finish()
}

struct DialogueLineAcceptanceTransaction {
    first_by_id: BTreeMap<DialogueLineId, DialogueLineCollisionSite>,
    accepted: Vec<AcceptedDialogueLine>,
    collisions: Vec<DialogueLineDiagnostic>,
    work: u32,
    candidate_count: usize,
}

impl DialogueLineAcceptanceTransaction {
    fn new() -> Self {
        Self {
            first_by_id: BTreeMap::new(),
            accepted: Vec::new(),
            collisions: Vec::new(),
            work: 0,
            candidate_count: 0,
        }
    }

    fn accept_module(
        &mut self,
        module: &HirProjectModule,
    ) -> Result<(), super::HirProjectBuildError> {
        let inventory = module.module().dialogue_line_candidates();
        if inventory.module() != module.module().key() {
            return Err(DialogueLineProjectFatal::SourceIdentityMismatch {
                module: Box::new(module.module().key().clone()),
                expected: Box::new(module.source().clone()),
                actual: Box::new(inventory.module().source().clone()),
            }
            .into());
        }

        let mut candidates = inventory.records().iter().collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate_source_key(candidate));
        if inventory
            .records()
            .iter()
            .zip(candidates.iter().copied())
            .any(|(original, sorted)| !core::ptr::eq(original, sorted))
        {
            return Err(DialogueLineProjectFatal::InvalidSourceOrder {
                module: module.module().key().clone(),
            }
            .into());
        }

        for candidate in candidates {
            self.accept_candidate(module, candidate)?;
        }
        Ok(())
    }

    fn accept_candidate(
        &mut self,
        module: &HirProjectModule,
        candidate: &HirDialogueLineCandidate,
    ) -> Result<(), super::HirProjectBuildError> {
        self.candidate_count = self
            .candidate_count
            .checked_add(1)
            .ok_or(DialogueLineProjectFatal::IndexOverflow)?;
        if self.candidate_count > MAX_PROJECT_DIALOGUE_LINE_CANDIDATES {
            return Err(DialogueLineProjectFatal::CandidateLimit {
                observed: self.candidate_count,
                maximum: MAX_PROJECT_DIALOGUE_LINE_CANDIDATES,
            }
            .into());
        }
        self.charge_work(3)?;

        let site = candidate.site();
        let module_key = module.module().key();
        if site.application_span().source() != module_key.source() {
            return Err(DialogueLineProjectFatal::SourceIdentityMismatch {
                module: Box::new(module_key.clone()),
                expected: Box::new(module_key.source().clone()),
                actual: Box::new(site.application_span().source().clone()),
            }
            .into());
        }
        if site.application().module() != module.module().module_id() {
            return Err(DialogueLineProjectFatal::ForeignExpression {
                module: Box::new(module_key.clone()),
                expression: site.application(),
            }
            .into());
        }

        let collision_site = DialogueLineCollisionSite::new(
            module_key.clone(),
            site.application(),
            site.source_order(),
            site.id_coordinate_span()
                .unwrap_or(site.application_span())
                .clone(),
        );
        if let Some(first) = self.first_by_id.get(candidate.id()) {
            self.push_collision(DialogueLineDiagnostic::LineIdCollision {
                id: candidate.id().clone(),
                first: Box::new(first.clone()),
                conflicting: Box::new(collision_site),
            })?;
            return Ok(());
        }

        self.first_by_id
            .insert(candidate.id().clone(), collision_site);
        self.accepted.push(AcceptedDialogueLine {
            id: candidate.id().clone(),
            text_key: candidate.text_key().clone(),
            id_origin: candidate.id_origin(),
            text_key_origin: candidate.text_key_origin(),
            source: AcceptedDialogueLineSource {
                module: module_key.clone(),
                application: site.application(),
                owner: site.owner().clone(),
                named_scopes: Arc::clone(site.named_scopes()),
                source_order: site.source_order(),
                application_span: site.application_span().clone(),
                id_coordinate_span: site.id_coordinate_span().cloned(),
                text_key_coordinate_span: site.text_key_coordinate_span().cloned(),
            },
        });
        Ok(())
    }

    fn push_collision(
        &mut self,
        diagnostic: DialogueLineDiagnostic,
    ) -> Result<(), DialogueLineProjectFatal> {
        let observed = self
            .collisions
            .len()
            .checked_add(1)
            .ok_or(DialogueLineProjectFatal::IndexOverflow)?;
        if observed > MAX_PROJECT_DIALOGUE_LINE_DIAGNOSTICS {
            return Err(DialogueLineProjectFatal::DiagnosticLimit {
                observed,
                maximum: MAX_PROJECT_DIALOGUE_LINE_DIAGNOSTICS,
            });
        }
        self.collisions.push(diagnostic);
        Ok(())
    }

    fn charge_work(&mut self, units: u32) -> Result<(), DialogueLineProjectFatal> {
        self.work = self
            .work
            .checked_add(units)
            .ok_or(DialogueLineProjectFatal::WorkLimit {
                observed: u32::MAX,
                maximum: MAX_PROJECT_DIALOGUE_LINE_WORK,
            })?;
        if self.work > MAX_PROJECT_DIALOGUE_LINE_WORK {
            return Err(DialogueLineProjectFatal::WorkLimit {
                observed: self.work,
                maximum: MAX_PROJECT_DIALOGUE_LINE_WORK,
            });
        }
        Ok(())
    }

    fn finish(mut self) -> Result<AcceptedDialogueLineInventory, super::HirProjectBuildError> {
        if !self.collisions.is_empty() {
            self.collisions
                .sort_by(DialogueLineDiagnostic::compare_for_publication);
            self.collisions.dedup();
            return Err(super::HirProjectBuildError::DialogueLines(
                DialogueLineProjectRejection {
                    diagnostics: Arc::from(self.collisions),
                },
            ));
        }
        if self.accepted.is_empty() {
            return Ok(AcceptedDialogueLineInventory::empty());
        }

        let source_exprs = self
            .accepted
            .iter()
            .map(|record| record.source().application())
            .collect::<Vec<_>>();
        self.accepted.sort_by(|left, right| {
            left.id().cmp(right.id()).then_with(|| {
                accepted_source_key(left.source()).cmp(&accepted_source_key(right.source()))
            })
        });

        let mut by_id = BTreeMap::new();
        let mut by_expr = BTreeMap::new();
        for (offset, record) in self.accepted.iter().enumerate() {
            let index = DialogueLineIndex::try_from_offset(offset)?;
            if by_id.insert(record.id().clone(), index).is_some() {
                return Err(DialogueLineProjectFatal::IndexOverflow.into());
            }
            if by_expr
                .insert(record.source().application(), index)
                .is_some()
            {
                return Err(DialogueLineProjectFatal::DuplicateExpression {
                    expression: record.source().application(),
                }
                .into());
            }
        }
        let source_order = source_exprs
            .into_iter()
            .map(|expression| {
                by_expr
                    .get(&expression)
                    .copied()
                    .ok_or(DialogueLineProjectFatal::DuplicateExpression { expression })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let records = Arc::from(self.accepted);
        Ok(AcceptedDialogueLineInventory {
            cache_fingerprint: fingerprint_inventory(&records),
            records,
            by_id,
            by_expr,
            source_order: Arc::from(source_order),
        })
    }
}

fn fingerprint_inventory(records: &[AcceptedDialogueLine]) -> DialogueLineInventoryFingerprint {
    let mut encoder = InventoryFingerprintEncoder::new();
    encoder.usize(records.len());
    for record in records {
        encoder.line(record);
    }
    DialogueLineInventoryFingerprint(encoder.finish())
}

struct InventoryFingerprintEncoder {
    hasher: blake3::Hasher,
}

impl InventoryFingerprintEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(INVENTORY_FINGERPRINT_DOMAIN);
        Self { hasher }
    }

    fn finish(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }

    fn line(&mut self, record: &AcceptedDialogueLine) {
        self.string(record.id().as_str());
        self.string(record.text_key().as_str());
        self.u8(match record.id_origin() {
            DialogueLineIdOrigin::ExplicitAbsolute => 0,
            DialogueLineIdOrigin::ExplicitRelative => 1,
            DialogueLineIdOrigin::ExplicitFamilyRelative => 2,
            DialogueLineIdOrigin::Generated => 3,
        });
        self.u8(match record.text_key_origin() {
            DialogueTextKeyOrigin::Explicit => 0,
            DialogueTextKeyOrigin::Derived => 1,
        });
        self.source(record.source());
    }

    fn source(&mut self, source: &AcceptedDialogueLineSource) {
        let module = source.module();
        self.string(module.package().as_str());
        self.usize(module.path().segments().len());
        for segment in module.path().segments() {
            self.string(segment.as_str());
        }
        self.source_identity(module.source());
        self.bytes(&source.application().cache_fingerprint_input());
        self.owner(source.owner());
        self.usize(source.named_scopes().len());
        for scope in source.named_scopes() {
            self.bytes(&scope.scope().cache_fingerprint_input());
            self.string(scope.segment().as_str());
            self.span(scope.declaration());
        }
        self.u32(source.source_order().get());
        self.span(source.application_span());
        self.optional_span(source.id_coordinate_span());
        self.optional_span(source.text_key_coordinate_span());
    }

    fn owner(&mut self, owner: &HirDialogueLineSourceOwner) {
        match owner {
            HirDialogueLineSourceOwner::Flow(owner) => {
                self.u8(0);
                self.string(owner.id().as_str());
            }
            HirDialogueLineSourceOwner::Callable(owner) => {
                self.u8(1);
                self.string(owner.package().as_str());
                self.usize(owner.module().segments().len());
                for segment in owner.module().segments() {
                    self.string(segment.as_str());
                }
                self.u8(owner.owner().digest_tag());
                self.usize(owner.owner_path().len());
                for segment in owner.owner_path() {
                    self.string(segment.as_str());
                }
                self.string(owner.name());
            }
            HirDialogueLineSourceOwner::Ownerless => self.u8(2),
        }
    }

    fn optional_span(&mut self, span: Option<&SourceSpan>) {
        match span {
            Some(span) => {
                self.u8(1);
                self.span(span);
            }
            None => self.u8(0),
        }
    }

    fn span(&mut self, span: &SourceSpan) {
        self.source_identity(span.source());
        self.usize(span.range().start());
        self.usize(span.range().end());
    }

    fn source_identity(&mut self, source: &SourceDocumentIdentity) {
        self.string(source.id().as_str());
        self.bytes(source.revision().as_bytes());
        self.u64(source.source_len());
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.usize(value.len());
        self.hasher.update(value);
    }

    fn usize(&mut self, value: usize) {
        self.u64(u64::try_from(value).expect("bounded HIR inventory lengths fit u64"));
    }

    fn u64(&mut self, value: u64) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn u8(&mut self, value: u8) {
        self.hasher.update(&[value]);
    }
}

fn candidate_source_key(
    candidate: &HirDialogueLineCandidate,
) -> (usize, usize, DialogueLineSourceOrder, ExprId) {
    let site = candidate.site();
    (
        site.application_span().range().start(),
        site.application_span().range().end(),
        site.source_order(),
        site.application(),
    )
}

fn accepted_source_key(
    source: &AcceptedDialogueLineSource,
) -> (&HirModuleKey, usize, usize, DialogueLineSourceOrder, ExprId) {
    (
        source.module(),
        source.application_span().range().start(),
        source.application_span().range().end(),
        source.source_order(),
        source.application(),
    )
}
