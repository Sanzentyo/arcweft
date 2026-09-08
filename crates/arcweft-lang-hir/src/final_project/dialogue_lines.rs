use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use super::HirSelectedExpressionGraph;
use crate::identity::ExprId;
use crate::leaf::HirIdRef;
use crate::line_identity::{
    DialogueIdentityCoordinateKind, DialogueLineCollisionSite, DialogueLineDiagnostic,
    DialogueLineIdOrigin, DialogueLineSourceOrder, DialogueTextKeyOrigin, HirDialogueLineCandidate,
    HirDialogueLineSite, HirDialogueLineSiteTopology, HirDialogueLineSourceOwner,
    HirDialogueNamedScope, InvalidCoordinateReason,
};
use crate::lowering::HirModuleKey;
use crate::module::HirModule;

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
    source_application: ExprId,
    semantic_application: ExprId,
    topology: HirDialogueLineSiteTopology,
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

    pub const fn source_application(&self) -> ExprId {
        self.source_application
    }

    pub const fn semantic_application(&self) -> ExprId {
        self.semantic_application
    }

    pub const fn topology(&self) -> HirDialogueLineSiteTopology {
        self.topology
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
    by_source_expr: BTreeMap<ExprId, DialogueLineIndex>,
    by_semantic_expr: BTreeMap<ExprId, DialogueLineIndex>,
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
            by_source_expr: BTreeMap::new(),
            by_semantic_expr: BTreeMap::new(),
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

    pub fn for_source_expr(&self, expr: ExprId) -> Option<&AcceptedDialogueLine> {
        self.by_source_expr
            .get(&expr)
            .map(|index| &self.records[index.offset()])
    }

    pub fn for_semantic_expr(&self, expr: ExprId) -> Option<&AcceptedDialogueLine> {
        self.by_semantic_expr
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

/// Failure returned by the post-selection dialogue-line seal. The HIR
/// project itself remains unchanged on either branch.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueLineProjectError {
    #[error(transparent)]
    Rejected(#[from] DialogueLineProjectRejection),
    #[error(transparent)]
    Fatal(#[from] DialogueLineProjectFatal),
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
    #[error("selected dialogue-line site topology is inconsistent for {expression:?}")]
    SelectionTopologyMismatch { expression: ExprId },
    #[error(transparent)]
    Build(#[from] crate::line_identity::DialogueLineBuildFatal),
}

/// Materializes and accepts only dialogue sites whose semantic application is
/// present in the sealed selected-expression graph.
pub(crate) fn accept_selected_dialogue_lines<'module>(
    modules: impl Iterator<Item = &'module HirProjectModule>,
    selected: &HirSelectedExpressionGraph,
) -> Result<AcceptedDialogueLineInventory, DialogueLineProjectError> {
    let mut transaction = DialogueLineAcceptanceTransaction::new();
    for module in modules {
        transaction.accept_module(module, selected)?;
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

fn validate_selected_site(
    module: &HirModule,
    site: &HirDialogueLineSite,
    selected: &HirSelectedExpressionGraph,
) -> Result<(), DialogueLineProjectFatal> {
    if !selected.contains_expression(site.semantic_application()) {
        return Ok(());
    }
    match site.topology() {
        HirDialogueLineSiteTopology::Direct => {
            if site.source_application() != site.semantic_application() {
                return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
                    expression: site.source_application(),
                });
            }
        }
        HirDialogueLineSiteTopology::OuterPostfixBracket { index_candidate } => {
            let outer = module
                .resolve_expr(site.source_application())
                .map_err(|_| DialogueLineProjectFatal::SelectionTopologyMismatch {
                    expression: site.source_application(),
                })?;
            let crate::expr::HirExprKind::PostfixBracket(postfix) = outer.kind() else {
                return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
                    expression: site.source_application(),
                });
            };
            let crate::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                index,
                dialogue,
            } = postfix.candidates()
            else {
                return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
                    expression: site.source_application(),
                });
            };
            if *index != index_candidate || *dialogue != site.semantic_application() {
                return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
                    expression: site.source_application(),
                });
            }
            let has_dialogue_edge = selected
                .expression_edges(site.source_application())
                .iter()
                .any(|edge| {
                    matches!(
                        edge,
                        crate::project::HirExpressionEvaluationEdge::Expression {
                            role: crate::expr::HirExpressionChildRole::PostfixDialogueCandidate,
                            child,
                            ..
                        } if *child == site.semantic_application()
                    )
                });
            if !has_dialogue_edge {
                return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
                    expression: site.source_application(),
                });
            }
        }
    }
    Ok(())
}

fn coordinate_reference(
    module: &HirModule,
    site: &HirDialogueLineSite,
    kind: crate::dialogue_application::HirDialogueCoordinateKind,
) -> Result<CoordinateEvidence, DialogueLineProjectFatal> {
    let expression = module
        .resolve_expr(site.semantic_application())
        .map_err(|_| DialogueLineProjectFatal::SelectionTopologyMismatch {
            expression: site.semantic_application(),
        })?;
    let crate::expr::HirExprKind::AttachedContentApplication(application) = expression.kind()
    else {
        return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
            expression: site.semantic_application(),
        });
    };
    let crate::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
        coordinates,
        ..
    } = application.family()
    else {
        return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
            expression: site.semantic_application(),
        });
    };
    let mut matches = coordinates
        .iter()
        .filter(|coordinate| coordinate.kind() == kind);
    let Some(first) = matches.next() else {
        return Ok(CoordinateEvidence::Absent);
    };
    if matches.next().is_some() {
        let Some(duplicate) = coordinates
            .iter()
            .filter(|coordinate| coordinate.kind() == kind)
            .nth(1)
        else {
            return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
                expression: site.semantic_application(),
            });
        };
        return Ok(CoordinateEvidence::Invalid(
            DialogueLineDiagnostic::DuplicateLineIdentityCoordinate {
                coordinate: coordinate_kind(kind),
                first: whole_expression_span(module, first.value())?,
                duplicate: whole_expression_span(module, duplicate.value())?,
            },
        ));
    }
    let value = module.dialogue_coordinate_value(first).map_err(|_| {
        DialogueLineProjectFatal::SelectionTopologyMismatch {
            expression: first.value(),
        }
    })?;
    let span = whole_expression_span(module, first.value())?;
    match value {
        crate::dialogue_application::HirDialogueCoordinateValueRef::IdRef(reference) => {
            Ok(CoordinateEvidence::Resolved(reference))
        }
        crate::dialogue_application::HirDialogueCoordinateValueRef::Error(_) => {
            Ok(CoordinateEvidence::Recovered)
        }
        crate::dialogue_application::HirDialogueCoordinateValueRef::Runtime(_) => Ok(
            CoordinateEvidence::Invalid(DialogueLineDiagnostic::InvalidLineIdentityCoordinate {
                coordinate: coordinate_kind(kind),
                reason: InvalidCoordinateReason::RuntimeExpression,
                span,
            }),
        ),
    }
}

enum CoordinateEvidence {
    Absent,
    Resolved(HirIdRef),
    Recovered,
    Invalid(DialogueLineDiagnostic),
}

impl CoordinateEvidence {
    const fn reference(&self) -> Option<&HirIdRef> {
        match self {
            Self::Resolved(reference) => Some(reference),
            Self::Absent | Self::Recovered | Self::Invalid(_) => None,
        }
    }

    const fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered)
    }

    fn diagnostic(&self) -> Option<&DialogueLineDiagnostic> {
        match self {
            Self::Invalid(diagnostic) => Some(diagnostic),
            Self::Absent | Self::Resolved(_) | Self::Recovered => None,
        }
    }
}

const fn coordinate_kind(
    kind: crate::dialogue_application::HirDialogueCoordinateKind,
) -> DialogueIdentityCoordinateKind {
    match kind {
        crate::dialogue_application::HirDialogueCoordinateKind::Id => {
            DialogueIdentityCoordinateKind::LineId
        }
        crate::dialogue_application::HirDialogueCoordinateKind::TextKey => {
            DialogueIdentityCoordinateKind::TextKey
        }
    }
}

fn whole_expression_span(
    module: &HirModule,
    owner: ExprId,
) -> Result<SourceSpan, DialogueLineProjectFatal> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            crate::source_index::HirSourceQuery::Expr {
                owner,
                role: crate::source_index::HirExprSourceRole::Whole,
            },
        )
        .map_err(|_| DialogueLineProjectFatal::SelectionTopologyMismatch { expression: owner })?;
    match lookup.presence() {
        crate::source_index::HirSourcePresence::Present(
            crate::source_index::HirSourceSite::Span(span),
        ) => Ok(span.clone()),
        crate::source_index::HirSourcePresence::Present(
            crate::source_index::HirSourceSite::Insertion(_),
        )
        | crate::source_index::HirSourcePresence::AbsentOptional => {
            Err(DialogueLineProjectFatal::SelectionTopologyMismatch { expression: owner })
        }
    }
}

fn dialogue_application_has_recovery(
    module: &HirModule,
    site: &HirDialogueLineSite,
) -> Result<bool, DialogueLineProjectFatal> {
    let expression = module
        .resolve_expr(site.semantic_application())
        .map_err(|_| DialogueLineProjectFatal::SelectionTopologyMismatch {
            expression: site.semantic_application(),
        })?;
    let crate::expr::HirExprKind::AttachedContentApplication(application) = expression.kind()
    else {
        return Err(DialogueLineProjectFatal::SelectionTopologyMismatch {
            expression: site.semantic_application(),
        });
    };
    Ok(application.has_recovery())
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
        selected: &HirSelectedExpressionGraph,
    ) -> Result<(), DialogueLineProjectError> {
        let inventory = module.module().dialogue_line_sites();
        if inventory.module() != module.module().key() {
            return Err(DialogueLineProjectFatal::SourceIdentityMismatch {
                module: Box::new(module.module().key().clone()),
                expected: Box::new(module.source().clone()),
                actual: Box::new(inventory.module().source().clone()),
            }
            .into());
        }

        let mut builder = crate::line_identity::builder::HirDialogueLineCandidateBuilder::new(
            module.module().key(),
        );
        for site in inventory
            .source_ordered()
            .filter(|site| selected.contains_expression(site.semantic_application()))
        {
            validate_selected_site(module.module(), site, selected)?;
            if dialogue_application_has_recovery(module.module(), site)? {
                builder.skip(site).map_err(DialogueLineProjectFatal::from)?;
                continue;
            }
            let id = coordinate_reference(
                module.module(),
                site,
                crate::dialogue_application::HirDialogueCoordinateKind::Id,
            )?;
            let text_key = coordinate_reference(
                module.module(),
                site,
                crate::dialogue_application::HirDialogueCoordinateKind::TextKey,
            )?;
            if let Some(diagnostic) = id.diagnostic().or_else(|| text_key.diagnostic()) {
                builder
                    .reject(site, diagnostic.clone())
                    .map_err(DialogueLineProjectFatal::from)?;
                continue;
            }
            if id.is_recovered() || text_key.is_recovered() {
                builder.skip(site).map_err(DialogueLineProjectFatal::from)?;
                continue;
            }
            builder
                .push(site.clone(), id.reference(), text_key.reference())
                .map_err(DialogueLineProjectFatal::from)?;
        }
        let (candidates, diagnostics) = builder.finish();
        for diagnostic in diagnostics.iter().cloned() {
            self.push_collision(diagnostic)?;
        }
        for candidate in &candidates {
            self.accept_candidate(module, candidate)?;
        }
        Ok(())
    }

    fn accept_candidate(
        &mut self,
        module: &HirProjectModule,
        candidate: &HirDialogueLineCandidate,
    ) -> Result<(), DialogueLineProjectError> {
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
        if site.source_application().module() != module.module().module_id()
            || site.semantic_application().module() != module.module().module_id()
        {
            return Err(DialogueLineProjectFatal::ForeignExpression {
                module: Box::new(module_key.clone()),
                expression: site.source_application(),
            }
            .into());
        }

        let collision_site = DialogueLineCollisionSite::new(
            module_key.clone(),
            site.source_application(),
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
                source_application: site.source_application(),
                semantic_application: site.semantic_application(),
                topology: site.topology(),
                owner: site.owner().clone(),
                named_scopes: Arc::from(site.named_scopes()),
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

    fn finish(mut self) -> Result<AcceptedDialogueLineInventory, DialogueLineProjectError> {
        if !self.collisions.is_empty() {
            self.collisions
                .sort_by(DialogueLineDiagnostic::compare_for_publication);
            self.collisions.dedup();
            return Err(DialogueLineProjectRejection {
                diagnostics: Arc::from(self.collisions),
            }
            .into());
        }
        if self.accepted.is_empty() {
            return Ok(AcceptedDialogueLineInventory::empty());
        }

        let source_exprs = self
            .accepted
            .iter()
            .map(|record| record.source().source_application())
            .collect::<Vec<_>>();
        let semantic_exprs = self
            .accepted
            .iter()
            .map(|record| record.source().semantic_application())
            .collect::<Vec<_>>();
        self.accepted.sort_by(|left, right| {
            left.id().cmp(right.id()).then_with(|| {
                accepted_source_key(left.source()).cmp(&accepted_source_key(right.source()))
            })
        });

        let mut by_id = BTreeMap::new();
        let mut by_source_expr = BTreeMap::new();
        let mut by_semantic_expr = BTreeMap::new();
        for (offset, record) in self.accepted.iter().enumerate() {
            let index = DialogueLineIndex::try_from_offset(offset)?;
            if by_id.insert(record.id().clone(), index).is_some() {
                return Err(DialogueLineProjectFatal::IndexOverflow.into());
            }
            if by_source_expr
                .insert(record.source().source_application(), index)
                .is_some()
            {
                return Err(DialogueLineProjectFatal::DuplicateExpression {
                    expression: record.source().source_application(),
                }
                .into());
            }
            if by_semantic_expr
                .insert(record.source().semantic_application(), index)
                .is_some()
            {
                return Err(DialogueLineProjectFatal::DuplicateExpression {
                    expression: record.source().semantic_application(),
                }
                .into());
            }
        }
        let source_order = source_exprs
            .into_iter()
            .map(|expression| {
                by_source_expr
                    .get(&expression)
                    .copied()
                    .ok_or(DialogueLineProjectFatal::DuplicateExpression { expression })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let records = Arc::from(self.accepted);
        let _semantic_exprs = semantic_exprs;
        Ok(AcceptedDialogueLineInventory {
            cache_fingerprint: fingerprint_inventory(&records),
            records,
            by_id,
            by_source_expr,
            by_semantic_expr,
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
        self.bytes(&source.source_application().cache_fingerprint_input());
        self.bytes(&source.semantic_application().cache_fingerprint_input());
        match source.topology() {
            HirDialogueLineSiteTopology::Direct => self.u8(0),
            HirDialogueLineSiteTopology::OuterPostfixBracket { index_candidate } => {
                self.u8(1);
                self.bytes(&index_candidate.cache_fingerprint_input());
            }
        }
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

fn accepted_source_key(
    source: &AcceptedDialogueLineSource,
) -> (&HirModuleKey, usize, usize, DialogueLineSourceOrder, ExprId) {
    (
        source.module(),
        source.application_span().range().start(),
        source.application_span().range().end(),
        source.source_order(),
        source.source_application(),
    )
}
