//! Immutable checked callable authority and its private transactional builder.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use arcweft_lang_hir::symbol::{
    CallableDeclarationKey, ProjectSymbolRevision, ProjectSymbolWorldId,
};
use arcweft_source::{SourceDocumentIdentity, SourceRange, SourceSpan};

use crate::{
    effect_row::{EffectRow, EffectRowError, EffectRowTail, EffectSubsetError, EffectSubstitution},
    effects::EffectSet,
    final_analysis::CheckedFunctionExecution,
    nominal::TypeSourceEvidence,
};

use super::{
    CallableAccess, CallableCandidateId, CallableEffectSchema, CallableRecord,
    CheckedCallableContext, CheckedCallableDeclaration, CheckedCallableId, CheckedClosureId,
    EnvironmentCallablePublicationDigest, ReceiverMethodKey, RegisteredCallableCatalog,
    RegisteredCallableCatalogDigest, StandardTraitCatalogVersion,
};

/// Exact generation shared by every record in one frozen checked catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableCatalogGeneration {
    origin: CheckedCallableCatalogOrigin,
    standard: StandardTraitCatalogVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallableCatalogOrigin {
    RegisteredProject {
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
        catalog: RegisteredCallableCatalogDigest,
    },
    Detached {
        source: SourceDocumentIdentity,
    },
}

/// Runtime disposition frozen after semantic role checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallableExecution {
    Runtime(CheckedFunctionExecution),
    DispatchContract,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectPermission {
    UnboundedInference,
    Bounded(EffectRow),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EffectContractOrigin {
    BodyInference,
    Authored,
    OmittedBodylessTraitRequirement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectItemSource {
    effect: crate::effects::EffectId,
    span: SourceSpan,
}

impl EffectItemSource {
    pub(crate) fn new(effect: crate::effects::EffectId, span: SourceSpan) -> Self {
        Self { effect, span }
    }

    pub const fn effect(&self) -> &crate::effects::EffectId {
        &self.effect
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectClauseSource {
    whole: SourceSpan,
    keyword: SourceSpan,
    items: Box<[EffectItemSource]>,
}

impl EffectClauseSource {
    pub(crate) fn try_new(
        whole: SourceSpan,
        keyword: SourceSpan,
        items: Box<[EffectItemSource]>,
    ) -> Result<Self, EffectContractBuildError> {
        if keyword.source() != whole.source()
            || items
                .iter()
                .any(|item| item.span.source() != whole.source())
        {
            return Err(EffectContractBuildError::SourceIdentityMismatch);
        }
        let whole_range = whole.range();
        let contains = |span: &SourceSpan| {
            let range = span.range();
            whole_range.start() <= range.start() && range.end() <= whole_range.end()
        };
        if !contains(&keyword) || items.iter().any(|item| !contains(&item.span)) {
            return Err(EffectContractBuildError::SpanOutsideClause);
        }
        Ok(Self {
            whole,
            keyword,
            items,
        })
    }

    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }

    pub const fn keyword(&self) -> &SourceSpan {
        &self.keyword
    }

    pub const fn items(&self) -> &[EffectItemSource] {
        &self.items
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectContractSource {
    origin: EffectContractOrigin,
    anchor: SourceSpan,
    clauses: Box<[EffectClauseSource]>,
    typed_tail_source: Option<TypeSourceEvidence>,
    forbidden_sources: Box<[EffectItemSource]>,
}

impl EffectContractSource {
    pub const fn origin(&self) -> EffectContractOrigin {
        self.origin
    }

    pub const fn anchor(&self) -> &SourceSpan {
        &self.anchor
    }

    pub const fn clauses(&self) -> &[EffectClauseSource] {
        &self.clauses
    }

    pub const fn typed_tail_source(&self) -> Option<&TypeSourceEvidence> {
        self.typed_tail_source.as_ref()
    }

    pub const fn forbidden_sources(&self) -> &[EffectItemSource] {
        &self.forbidden_sources
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableEffectContract {
    permission: EffectPermission,
    forbidden: EffectSet,
    source: EffectContractSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectContractBuildError {
    MissingAuthoredSource,
    UnknownAuthoredTail,
    SourceIdentityMismatch,
    SpanOutsideClause,
}

impl CallableEffectContract {
    pub(crate) fn body_inference(
        anchor: SourceSpan,
        forbidden: EffectSet,
        forbidden_sources: Box<[EffectItemSource]>,
    ) -> Result<Self, EffectContractBuildError> {
        validate_effect_sources(&anchor, &[], &forbidden_sources)?;
        Ok(Self {
            permission: EffectPermission::UnboundedInference,
            forbidden,
            source: EffectContractSource {
                origin: EffectContractOrigin::BodyInference,
                anchor,
                clauses: Box::new([]),
                typed_tail_source: None,
                forbidden_sources,
            },
        })
    }

    pub(crate) fn authored(
        row: EffectRow,
        clauses: Box<[EffectClauseSource]>,
        typed_tail_source: Option<TypeSourceEvidence>,
        forbidden: EffectSet,
        forbidden_sources: Box<[EffectItemSource]>,
    ) -> Result<Self, EffectContractBuildError> {
        if clauses.is_empty() && typed_tail_source.is_none() {
            return Err(EffectContractBuildError::MissingAuthoredSource);
        }
        if matches!(row.tail(), EffectRowTail::Unknown) {
            return Err(EffectContractBuildError::UnknownAuthoredTail);
        }
        let anchor = clauses
            .first()
            .map(|clause| clause.whole.clone())
            .or_else(|| {
                typed_tail_source
                    .as_ref()
                    .and_then(TypeSourceEvidence::project)
                    .cloned()
            })
            .ok_or(EffectContractBuildError::MissingAuthoredSource)?;
        validate_effect_sources(&anchor, &clauses, &forbidden_sources)?;
        Ok(Self {
            permission: EffectPermission::Bounded(row),
            forbidden,
            source: EffectContractSource {
                origin: EffectContractOrigin::Authored,
                anchor,
                clauses,
                typed_tail_source,
                forbidden_sources,
            },
        })
    }

    pub(crate) fn omitted_bodyless_trait(method_name: SourceSpan) -> Self {
        Self {
            permission: EffectPermission::Bounded(EffectRow::closed(EffectSet::new())),
            forbidden: EffectSet::new(),
            source: EffectContractSource {
                origin: EffectContractOrigin::OmittedBodylessTraitRequirement,
                anchor: method_name,
                clauses: Box::new([]),
                typed_tail_source: None,
                forbidden_sources: Box::new([]),
            },
        }
    }

    pub const fn permission(&self) -> &EffectPermission {
        &self.permission
    }

    pub const fn forbidden(&self) -> &EffectSet {
        &self.forbidden
    }

    pub const fn source(&self) -> &EffectContractSource {
        &self.source
    }

    fn exposed_or<'a>(&'a self, inferred: &'a EffectRow) -> &'a EffectRow {
        match &self.permission {
            EffectPermission::UnboundedInference => inferred,
            EffectPermission::Bounded(row) => row,
        }
    }
}

fn validate_effect_sources(
    anchor: &SourceSpan,
    clauses: &[EffectClauseSource],
    forbidden: &[EffectItemSource],
) -> Result<(), EffectContractBuildError> {
    let source = anchor.source();
    let valid = clauses.iter().all(|clause| {
        clause.whole.source() == source
            && clause.keyword.source() == source
            && clause.items.iter().all(|item| item.span.source() == source)
    }) && forbidden.iter().all(|item| item.span.source() == source);
    if valid {
        Ok(())
    } else {
        Err(EffectContractBuildError::SourceIdentityMismatch)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallableEffects {
    Body {
        contract: CallableEffectContract,
        inferred: EffectRow,
    },
    BodylessTraitRequirement {
        contract: CallableEffectContract,
    },
    RecordFixed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableInterfaceDigest([u8; 32]);

impl CallableInterfaceDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Sole checked metadata/effect record for one accepted callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableFacts {
    id: CheckedCallableId,
    record: Arc<CallableRecord>,
    execution: CheckedCallableExecution,
    effects: CheckedCallableEffects,
    interface_digest: CallableInterfaceDigest,
}

impl CheckedCallableFacts {
    pub const fn id(&self) -> &CheckedCallableId {
        &self.id
    }

    pub const fn record(&self) -> &Arc<CallableRecord> {
        &self.record
    }

    pub fn signature(&self) -> &super::CallableSignatureSchema {
        self.record.schema()
    }

    pub fn source(&self) -> Option<&super::CallableSource> {
        self.record.source()
    }

    pub fn documentation(&self) -> &super::CallableDocumentation {
        self.record.documentation()
    }

    pub fn access(&self) -> &CallableAccess {
        self.record.access()
    }

    pub fn provider(&self) -> &super::CallableProviderId {
        self.record.provider()
    }

    pub fn publication_digest(&self) -> Option<EnvironmentCallablePublicationDigest> {
        self.record.publication_digest()
    }

    pub const fn execution(&self) -> &CheckedCallableExecution {
        &self.execution
    }

    pub const fn actual_row(&self) -> Option<&EffectRow> {
        match &self.effects {
            CheckedCallableEffects::Body { inferred, .. } => Some(inferred),
            CheckedCallableEffects::BodylessTraitRequirement { .. }
            | CheckedCallableEffects::RecordFixed => None,
        }
    }

    /// Returns the sole source-contract provenance for source-backed checked
    /// effects. Fixed accepted rows remain record-owned and therefore have no
    /// checked contract origin.
    pub const fn effect_contract_origin(&self) -> Option<EffectContractOrigin> {
        match &self.effects {
            CheckedCallableEffects::Body { contract, .. }
            | CheckedCallableEffects::BodylessTraitRequirement { contract } => {
                Some(contract.source.origin)
            }
            CheckedCallableEffects::RecordFixed => None,
        }
    }

    /// Returns the effective checked row published to consumers.
    ///
    /// # Panics
    ///
    /// Panics only if a value bypassed catalog admission and claims a
    /// bodyless unbounded contract or a fixed record without a fixed row.
    pub fn exposed_row(&self) -> &EffectRow {
        match &self.effects {
            CheckedCallableEffects::Body { contract, inferred } => contract.exposed_or(inferred),
            CheckedCallableEffects::BodylessTraitRequirement { contract } => {
                match contract.permission() {
                    EffectPermission::Bounded(row) => row,
                    EffectPermission::UnboundedInference => unreachable!(
                        "bodyless trait requirements are admitted only with a bounded contract"
                    ),
                }
            }
            CheckedCallableEffects::RecordFixed => self
                .record
                .schema()
                .effects()
                .fixed_row()
                .expect("RecordFixed admission validates a fixed effect schema"),
        }
    }

    pub const fn interface_digest(&self) -> CallableInterfaceDigest {
        self.interface_digest
    }
}

/// Terminal result of one checked receiver/member lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedMethodLookup {
    Absent,
    Unique(Box<CheckedCallableId>),
    Ambiguous(Arc<[CheckedCallableId]>),
    Inaccessible(Arc<[CheckedCallableId]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallableLookupError {
    Missing,
    CandidateMismatch,
    RecordPointerMismatch,
    WrongFamily,
    ForeignWorld,
    StaleProjectRevision,
    ForeignCatalogDigest,
    StaleStandardVersion,
    ForeignDetachedSource,
}

/// Typed source component used by the checked callable source index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallableSourceCategory {
    Declaration,
    Name,
    Signature,
    EffectContract,
    Body,
}

/// Revision-bound source key for one accepted checked callable.
///
/// This key retains no spelling and can only be constructed from a bound
/// source span.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableSourceKey {
    source: SourceDocumentIdentity,
    category: CheckedCallableSourceCategory,
    range: SourceRange,
}

impl CheckedCallableSourceKey {
    #[must_use]
    pub fn from_span(category: CheckedCallableSourceCategory, span: &SourceSpan) -> Self {
        Self {
            source: span.source().clone(),
            category,
            range: span.range(),
        }
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    pub const fn category(&self) -> CheckedCallableSourceCategory {
        self.category
    }

    pub const fn range(&self) -> SourceRange {
        self.range
    }
}

/// Immutable checked callable and method-resolution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableCatalog {
    generation: CheckedCallableCatalogGeneration,
    registered: Option<Arc<RegisteredCallableCatalog>>,
    records: BTreeMap<CheckedCallableId, CheckedCallableFacts>,
    checked_by_candidate: HashMap<CallableCandidateId, CheckedCallableId>,
    method_index: HashMap<ReceiverMethodKey, Arc<[CheckedCallableId]>>,
    inaccessible_methods: HashMap<ReceiverMethodKey, Arc<[CheckedCallableId]>>,
    closure_rows: BTreeMap<CheckedClosureId, EffectRow>,
    closure_source_index: BTreeMap<SourceSpan, CheckedClosureId>,
    source_index: BTreeMap<CheckedCallableSourceKey, CheckedCallableId>,
}

impl CheckedCallableCatalog {
    pub const fn generation(&self) -> &CheckedCallableCatalogGeneration {
        &self.generation
    }

    pub const fn registered_catalog(&self) -> Option<&Arc<RegisteredCallableCatalog>> {
        self.registered.as_ref()
    }

    /// Validates the exact registered authority retained by this checked
    /// generation. Equal catalog values from another allocation are not an
    /// accepted lease.
    pub fn validate_registered_authority(
        &self,
        registered: &Arc<RegisteredCallableCatalog>,
        world: &ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
    ) -> Result<(), CheckedCallableLookupError> {
        let Some(retained) = &self.registered else {
            return Err(CheckedCallableLookupError::WrongFamily);
        };
        if !Arc::ptr_eq(retained, registered) {
            return Err(CheckedCallableLookupError::RecordPointerMismatch);
        }
        match &self.generation.origin {
            CheckedCallableCatalogOrigin::RegisteredProject {
                world: retained_world,
                revision: retained_revision,
                catalog,
            } if retained_world == world
                && *retained_revision == revision
                && *catalog == registered.digest() =>
            {
                Ok(())
            }
            CheckedCallableCatalogOrigin::RegisteredProject {
                world: retained_world,
                revision: retained_revision,
                catalog,
            } => {
                if retained_world != world {
                    Err(CheckedCallableLookupError::ForeignWorld)
                } else if *retained_revision != revision {
                    Err(CheckedCallableLookupError::StaleProjectRevision)
                } else if *catalog != registered.digest() {
                    Err(CheckedCallableLookupError::ForeignCatalogDigest)
                } else {
                    unreachable!("registered generation mismatch was exhaustively classified")
                }
            }
            CheckedCallableCatalogOrigin::Detached { .. } => {
                Err(CheckedCallableLookupError::WrongFamily)
            }
        }
    }

    /// Validates only the project generation carried by this immutable
    /// checked authority. Exact registered-pointer validation is performed at
    /// the analyzer publication boundary with
    /// [`Self::validate_registered_authority`].
    pub fn validate_project_generation(
        &self,
        world: &ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
    ) -> Result<(), CheckedCallableLookupError> {
        match &self.generation.origin {
            CheckedCallableCatalogOrigin::RegisteredProject {
                world: retained_world,
                revision: retained_revision,
                ..
            } => {
                if retained_world != world {
                    Err(CheckedCallableLookupError::ForeignWorld)
                } else if *retained_revision != revision {
                    Err(CheckedCallableLookupError::StaleProjectRevision)
                } else {
                    Ok(())
                }
            }
            CheckedCallableCatalogOrigin::Detached { .. } => {
                Err(CheckedCallableLookupError::WrongFamily)
            }
        }
    }

    /// Iterates the immutable checked facts in structural checked-ID order.
    /// Consumers must retain the IDs carried by the facts; display names are
    /// not an alternate lookup authority.
    pub fn records(
        &self,
    ) -> impl ExactSizeIterator<Item = &CheckedCallableFacts> + DoubleEndedIterator {
        self.records.values()
    }

    pub fn callable(
        &self,
        id: &CheckedCallableId,
    ) -> Result<&CheckedCallableFacts, CheckedCallableLookupError> {
        self.validate_context(id)?;
        self.records
            .get(id)
            .ok_or(CheckedCallableLookupError::Missing)
    }

    pub fn checked_for_candidate(
        &self,
        id: &CallableCandidateId,
    ) -> Result<&CheckedCallableId, CheckedCallableLookupError> {
        let checked = self
            .checked_by_candidate
            .get(id)
            .ok_or(CheckedCallableLookupError::Missing)?;
        self.validate_context(checked)?;
        Ok(checked)
    }

    /// Resolves one structural project declaration through the exact
    /// candidate-to-checked join. This is the declaration-owned consumer seam
    /// for compiler, Agent, and tooling projections; it performs no display
    /// name scan or accepted-record reconstruction.
    pub fn project_callable(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Result<&CheckedCallableFacts, CheckedCallableLookupError> {
        let candidate = CallableCandidateId::Project(declaration.clone());
        let checked = self.checked_for_candidate(&candidate)?;
        let facts = self.callable(checked)?;
        if facts.record().id() != &candidate
            || !matches!(
                checked.declaration(),
                CheckedCallableDeclaration::Project(retained) if retained == declaration
            )
        {
            return Err(CheckedCallableLookupError::CandidateMismatch);
        }
        Ok(facts)
    }

    pub fn callable_at_source(
        &self,
        key: &CheckedCallableSourceKey,
    ) -> Result<&CheckedCallableFacts, CheckedCallableLookupError> {
        let checked = self
            .source_index
            .get(key)
            .ok_or(CheckedCallableLookupError::Missing)?;
        self.validate_context(checked)?;
        self.records
            .get(checked)
            .ok_or(CheckedCallableLookupError::Missing)
    }

    pub fn method(&self, key: &ReceiverMethodKey) -> CheckedMethodLookup {
        if let Some(candidates) = self.inaccessible_methods.get(key) {
            return CheckedMethodLookup::Inaccessible(Arc::clone(candidates));
        }
        match self.method_index.get(key).map(AsRef::as_ref) {
            None | Some([]) => CheckedMethodLookup::Absent,
            Some([candidate]) => CheckedMethodLookup::Unique(Box::new(candidate.clone())),
            Some(candidates) => CheckedMethodLookup::Ambiguous(candidates.into()),
        }
    }

    pub fn closure_row(
        &self,
        id: &CheckedClosureId,
    ) -> Result<&EffectRow, CheckedCallableLookupError> {
        self.validate_context(id.owner())?;
        self.closure_rows
            .get(id)
            .ok_or(CheckedCallableLookupError::Missing)
    }

    /// Resolves the latent row of one closure through its exact accepted
    /// expression span. The source identity includes the document revision;
    /// equal offsets from another document or generation cannot match.
    pub fn closure_at_source(
        &self,
        source: &SourceSpan,
    ) -> Result<&EffectRow, CheckedCallableLookupError> {
        let id = self
            .closure_source_index
            .get(source)
            .ok_or(CheckedCallableLookupError::Missing)?;
        self.closure_row(id)
    }

    fn validate_context(&self, id: &CheckedCallableId) -> Result<(), CheckedCallableLookupError> {
        validate_checked_context(&self.generation, id)
    }
}

fn validate_checked_context(
    generation: &CheckedCallableCatalogGeneration,
    id: &CheckedCallableId,
) -> Result<(), CheckedCallableLookupError> {
    match (&generation.origin, id.context(), id.declaration()) {
        (
            CheckedCallableCatalogOrigin::RegisteredProject {
                world,
                revision,
                catalog,
            },
            CheckedCallableContext::Project {
                world: id_world,
                revision: id_revision,
                catalog: id_catalog,
                standard,
            },
            CheckedCallableDeclaration::Project(_),
        ) => {
            if world != id_world {
                Err(CheckedCallableLookupError::ForeignWorld)
            } else if revision != id_revision {
                Err(CheckedCallableLookupError::StaleProjectRevision)
            } else if catalog != id_catalog {
                Err(CheckedCallableLookupError::ForeignCatalogDigest)
            } else if &generation.standard != standard {
                Err(CheckedCallableLookupError::StaleStandardVersion)
            } else {
                Ok(())
            }
        }
        (
            CheckedCallableCatalogOrigin::RegisteredProject { catalog, .. },
            CheckedCallableContext::Environment {
                catalog: id_catalog,
            },
            CheckedCallableDeclaration::Environment(_),
        ) if catalog == id_catalog => Ok(()),
        (
            CheckedCallableCatalogOrigin::RegisteredProject { .. }
            | CheckedCallableCatalogOrigin::Detached { .. },
            CheckedCallableContext::Standard { version },
            CheckedCallableDeclaration::Standard(_),
        ) if &generation.standard == version => Ok(()),
        (
            CheckedCallableCatalogOrigin::Detached { source },
            CheckedCallableContext::Detached {
                source: id_source,
                standard,
            },
            CheckedCallableDeclaration::Detached(_),
        ) => {
            if source != id_source {
                Err(CheckedCallableLookupError::ForeignDetachedSource)
            } else if &generation.standard != standard {
                Err(CheckedCallableLookupError::StaleStandardVersion)
            } else {
                Ok(())
            }
        }
        (_, CheckedCallableContext::Environment { .. }, _) => {
            Err(CheckedCallableLookupError::ForeignCatalogDigest)
        }
        _ => Err(CheckedCallableLookupError::WrongFamily),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CheckedCallableCatalogBuildError {
    GenerationMismatch,
    CandidateMismatch,
    DuplicateCallable,
    DuplicateCandidate,
    MissingRegisteredRecord,
    RecordPointerMismatch,
    InvalidEffectAuthority,
    InvalidExecutionRole,
    MissingMethodRecord,
    CorruptIndex,
    DuplicateSource,
    SourceIdentityMismatch,
    InvalidState,
    MissingInference,
    DuplicateInference,
    DuplicateValidation,
    UnknownEffectRow,
    EffectRow(EffectRowError),
    EffectSubset(EffectSubsetError),
    ForbiddenEffects(EffectSet),
    ForeignClosureOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedCatalogBuildState {
    Collecting,
    Inferring,
    Validating,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingCallableEffectContract {
    Body(CallableEffectContract),
    BodylessTraitRequirement(CallableEffectContract),
    RecordFixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingCallableCompletion {
    AwaitingInference,
    AwaitingValidation,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCheckedCallable {
    id: CheckedCallableId,
    record: Arc<CallableRecord>,
    execution: CheckedCallableExecution,
    contract: PendingCallableEffectContract,
    inferred: Option<EffectRow>,
    completion: PendingCallableCompletion,
}

impl PendingCheckedCallable {
    pub(crate) const fn id(&self) -> &CheckedCallableId {
        &self.id
    }

    pub(crate) const fn record(&self) -> &Arc<CallableRecord> {
        &self.record
    }

    pub(crate) fn body_contract(&self) -> Option<&CallableEffectContract> {
        match &self.contract {
            PendingCallableEffectContract::Body(contract) => Some(contract),
            PendingCallableEffectContract::BodylessTraitRequirement(_)
            | PendingCallableEffectContract::RecordFixed => None,
        }
    }
}

/// Private, consuming construction transaction for a checked catalog.
pub(crate) struct CheckedCallableCatalogBuilder {
    generation: CheckedCallableCatalogGeneration,
    registered: Option<Arc<RegisteredCallableCatalog>>,
    state: CheckedCatalogBuildState,
    pending: BTreeMap<CheckedCallableId, PendingCheckedCallable>,
    checked_by_candidate: HashMap<CallableCandidateId, CheckedCallableId>,
    method_index: HashMap<ReceiverMethodKey, Vec<CheckedCallableId>>,
    closure_rows: BTreeMap<CheckedClosureId, EffectRow>,
    closure_source_index: BTreeMap<SourceSpan, CheckedClosureId>,
    source_index: BTreeMap<CheckedCallableSourceKey, CheckedCallableId>,
    effect_substitution: EffectSubstitution,
}

impl CheckedCallableCatalogBuilder {
    pub(crate) fn for_registered(
        registered: Arc<RegisteredCallableCatalog>,
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
        standard: StandardTraitCatalogVersion,
    ) -> Result<Self, CheckedCallableCatalogBuildError> {
        if registered.nominal_world().world() != &world
            || registered.nominal_world().revision() != revision
        {
            return Err(CheckedCallableCatalogBuildError::GenerationMismatch);
        }
        let catalog = registered.digest();
        Ok(Self {
            generation: CheckedCallableCatalogGeneration {
                origin: CheckedCallableCatalogOrigin::RegisteredProject {
                    world,
                    revision,
                    catalog,
                },
                standard,
            },
            registered: Some(registered),
            state: CheckedCatalogBuildState::Collecting,
            pending: BTreeMap::new(),
            checked_by_candidate: HashMap::new(),
            method_index: HashMap::new(),
            closure_rows: BTreeMap::new(),
            closure_source_index: BTreeMap::new(),
            source_index: BTreeMap::new(),
            effect_substitution: EffectSubstitution::new(),
        })
    }

    /// Clones the exact accepted record Arcs in deterministic structural
    /// identity order. The returned allocations remain pointer-identical to
    /// the registered authority and are revalidated at shell insertion and
    /// freeze.
    pub(crate) fn registered_records(
        &self,
    ) -> Result<Vec<Arc<CallableRecord>>, CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Collecting {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        let registered = self
            .registered
            .as_ref()
            .ok_or(CheckedCallableCatalogBuildError::GenerationMismatch)?;
        Ok(registered
            .records_in_identity_order()
            .into_iter()
            .map(Arc::clone)
            .collect())
    }

    pub(crate) fn insert_body_shell(
        &mut self,
        record: Arc<CallableRecord>,
        execution: CheckedCallableExecution,
        contract: CallableEffectContract,
        body_source: &SourceSpan,
    ) -> Result<CheckedCallableId, CheckedCallableCatalogBuildError> {
        self.insert_registered_shell(
            record,
            execution,
            PendingCallableEffectContract::Body(contract),
            Some(body_source),
        )
    }

    pub(crate) fn insert_bodyless_trait_shell(
        &mut self,
        record: Arc<CallableRecord>,
        execution: CheckedCallableExecution,
        contract: CallableEffectContract,
    ) -> Result<CheckedCallableId, CheckedCallableCatalogBuildError> {
        self.insert_registered_shell(
            record,
            execution,
            PendingCallableEffectContract::BodylessTraitRequirement(contract),
            None,
        )
    }

    pub(crate) fn insert_fixed_shell(
        &mut self,
        record: Arc<CallableRecord>,
        execution: CheckedCallableExecution,
    ) -> Result<CheckedCallableId, CheckedCallableCatalogBuildError> {
        self.insert_registered_shell(
            record,
            execution,
            PendingCallableEffectContract::RecordFixed,
            None,
        )
    }

    fn insert_registered_shell(
        &mut self,
        record: Arc<CallableRecord>,
        execution: CheckedCallableExecution,
        contract: PendingCallableEffectContract,
        body_source: Option<&SourceSpan>,
    ) -> Result<CheckedCallableId, CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Collecting {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        let registered = self
            .registered
            .as_ref()
            .ok_or(CheckedCallableCatalogBuildError::GenerationMismatch)?;
        let accepted = registered
            .record(record.id())
            .ok_or(CheckedCallableCatalogBuildError::MissingRegisteredRecord)?;
        if !Arc::ptr_eq(accepted, &record) {
            return Err(CheckedCallableCatalogBuildError::RecordPointerMismatch);
        }
        let id = match (
            &self.generation.origin,
            self.generation.standard,
            record.id(),
        ) {
            (
                CheckedCallableCatalogOrigin::RegisteredProject {
                    world,
                    revision,
                    catalog,
                },
                standard,
                CallableCandidateId::Project(declaration),
            ) => CheckedCallableId::for_project(
                world.clone(),
                *revision,
                *catalog,
                standard,
                declaration.clone(),
            )
            .map_err(|_| CheckedCallableCatalogBuildError::GenerationMismatch)?,
            (
                CheckedCallableCatalogOrigin::RegisteredProject { catalog, .. },
                _,
                CallableCandidateId::Environment(declaration),
            ) => CheckedCallableId::for_environment(*catalog, declaration.clone()),
            _ => return Err(CheckedCallableCatalogBuildError::CandidateMismatch),
        };
        self.insert_shell_with_id(record, id, execution, contract, body_source)
    }

    fn insert_shell_with_id(
        &mut self,
        record: Arc<CallableRecord>,
        id: CheckedCallableId,
        execution: CheckedCallableExecution,
        contract: PendingCallableEffectContract,
        body_source: Option<&SourceSpan>,
    ) -> Result<CheckedCallableId, CheckedCallableCatalogBuildError> {
        validate_checked_context(&self.generation, &id)
            .map_err(|_| CheckedCallableCatalogBuildError::GenerationMismatch)?;
        validate_pending_roles(&record, &execution, &contract)?;
        if self.pending.contains_key(&id) {
            return Err(CheckedCallableCatalogBuildError::DuplicateCallable);
        }
        if self.checked_by_candidate.contains_key(record.id()) {
            return Err(CheckedCallableCatalogBuildError::DuplicateCandidate);
        }
        let source_keys = checked_source_keys(&record, &contract, body_source);
        validate_source_membership(&record, &source_keys)?;
        if source_keys.iter().collect::<BTreeSet<_>>().len() != source_keys.len() {
            return Err(CheckedCallableCatalogBuildError::DuplicateSource);
        }
        if source_keys
            .iter()
            .any(|key| self.source_index.contains_key(key))
        {
            return Err(CheckedCallableCatalogBuildError::DuplicateSource);
        }
        let completion = if matches!(contract, PendingCallableEffectContract::Body(_)) {
            PendingCallableCompletion::AwaitingInference
        } else {
            PendingCallableCompletion::Complete
        };
        let candidate = record.id().clone();
        let pending = PendingCheckedCallable {
            id: id.clone(),
            record,
            execution,
            contract,
            inferred: None,
            completion,
        };
        let previous = self.pending.insert(id.clone(), pending);
        debug_assert!(
            previous.is_none(),
            "duplicate shell was rejected before mutation"
        );
        let previous = self.checked_by_candidate.insert(candidate, id.clone());
        debug_assert!(
            previous.is_none(),
            "duplicate candidate was rejected before mutation"
        );
        for key in source_keys {
            let previous = self.source_index.insert(key.clone(), id.clone());
            debug_assert!(
                previous.is_none(),
                "duplicate source was rejected before mutation"
            );
        }
        Ok(id)
    }

    pub(crate) fn begin_inference(&mut self) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Collecting {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        self.state = CheckedCatalogBuildState::Inferring;
        Ok(())
    }

    pub(crate) fn assign_inferred_row(
        &mut self,
        id: &CheckedCallableId,
        inferred: EffectRow,
    ) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Inferring {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        if !inferred.is_known() {
            return Err(CheckedCallableCatalogBuildError::UnknownEffectRow);
        }
        let pending = self
            .pending
            .get_mut(id)
            .ok_or(CheckedCallableCatalogBuildError::MissingInference)?;
        if !matches!(pending.contract, PendingCallableEffectContract::Body(_)) {
            return Err(CheckedCallableCatalogBuildError::InvalidEffectAuthority);
        }
        if pending.inferred.is_some() {
            return Err(CheckedCallableCatalogBuildError::DuplicateInference);
        }
        pending.inferred = Some(inferred);
        pending.completion = PendingCallableCompletion::AwaitingValidation;
        Ok(())
    }

    pub(crate) fn begin_validation(&mut self) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Inferring {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        if self
            .pending
            .values()
            .any(|pending| pending.completion == PendingCallableCompletion::AwaitingInference)
        {
            return Err(CheckedCallableCatalogBuildError::MissingInference);
        }
        self.state = CheckedCatalogBuildState::Validating;
        Ok(())
    }

    pub(crate) fn validate_body_contract(
        &mut self,
        id: &CheckedCallableId,
    ) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Validating {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        let (contract, inferred) = {
            let pending = self
                .pending
                .get(id)
                .ok_or(CheckedCallableCatalogBuildError::MissingInference)?;
            if pending.completion == PendingCallableCompletion::Complete {
                return Err(CheckedCallableCatalogBuildError::DuplicateValidation);
            }
            if pending.completion != PendingCallableCompletion::AwaitingValidation {
                return Err(CheckedCallableCatalogBuildError::MissingInference);
            }
            let PendingCallableEffectContract::Body(contract) = &pending.contract else {
                return Err(CheckedCallableCatalogBuildError::InvalidEffectAuthority);
            };
            let inferred = pending
                .inferred
                .as_ref()
                .ok_or(CheckedCallableCatalogBuildError::MissingInference)?;
            (contract.clone(), inferred.clone())
        };

        let mut staged_substitution = self.effect_substitution.clone();
        let actual = EffectRow::closed(
            inferred
                .resolve(&staged_substitution)
                .map_err(CheckedCallableCatalogBuildError::EffectRow)?,
        );
        if let EffectPermission::Bounded(permitted) = contract.permission() {
            EffectRow::check_subset(&actual, permitted, &mut staged_substitution)
                .map_err(CheckedCallableCatalogBuildError::EffectSubset)?;
        }
        let forbidden = actual.concrete().intersection(contract.forbidden());
        if !forbidden.is_empty() {
            return Err(CheckedCallableCatalogBuildError::ForbiddenEffects(
                forbidden,
            ));
        }

        self.effect_substitution = staged_substitution;
        let pending = self
            .pending
            .get_mut(id)
            .expect("validated pending callable remains in the transaction");
        pending.inferred = Some(actual);
        pending.completion = PendingCallableCompletion::Complete;
        Ok(())
    }

    pub(crate) fn pending_by_candidate(
        &self,
        candidate: &CallableCandidateId,
    ) -> Result<&PendingCheckedCallable, CheckedCallableLookupError> {
        let checked = self
            .checked_by_candidate
            .get(candidate)
            .ok_or(CheckedCallableLookupError::Missing)?;
        let pending = self.pending_by_id(checked)?;
        if pending.record.id() != candidate {
            return Err(CheckedCallableLookupError::CandidateMismatch);
        }
        Ok(pending)
    }

    pub(crate) fn pending_by_id(
        &self,
        id: &CheckedCallableId,
    ) -> Result<&PendingCheckedCallable, CheckedCallableLookupError> {
        validate_checked_context(&self.generation, id)?;
        let pending = self
            .pending
            .get(id)
            .ok_or(CheckedCallableLookupError::Missing)?;
        if pending.id != *id {
            return Err(CheckedCallableLookupError::CandidateMismatch);
        }
        if let Some(registered) = &self.registered {
            let accepted = registered
                .record(pending.record.id())
                .ok_or(CheckedCallableLookupError::Missing)?;
            if !Arc::ptr_eq(accepted, &pending.record) {
                return Err(CheckedCallableLookupError::RecordPointerMismatch);
            }
        }
        Ok(pending)
    }

    pub(crate) fn method(&self, key: &ReceiverMethodKey) -> CheckedMethodLookup {
        self.method_index
            .get(key)
            .map_or(CheckedMethodLookup::Absent, |candidates| {
                freeze_lookup(candidates)
            })
    }

    pub(crate) fn stage_method_candidate(
        &mut self,
        key: &ReceiverMethodKey,
        id: CheckedCallableId,
    ) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Collecting {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        let pending = self
            .pending
            .get(&id)
            .ok_or(CheckedCallableCatalogBuildError::MissingMethodRecord)?;
        let super::CallableLookupKey::Method(record_key) = pending.record.key() else {
            return Err(CheckedCallableCatalogBuildError::MissingMethodRecord);
        };
        if pending.record.method_role().is_none()
            || matches!(pending.record.access(), CallableAccess::TraitImplementation)
            || record_key != key
        {
            return Err(CheckedCallableCatalogBuildError::MissingMethodRecord);
        }
        let candidates = self.method_index.entry(key.clone()).or_default();
        if candidates.contains(&id) {
            return Err(CheckedCallableCatalogBuildError::DuplicateCallable);
        }
        candidates.push(id);
        Ok(())
    }

    pub(crate) fn insert_closure_row(
        &mut self,
        id: CheckedClosureId,
        row: EffectRow,
    ) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Inferring {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        let owner = self
            .pending
            .get(id.owner())
            .ok_or(CheckedCallableCatalogBuildError::ForeignClosureOwner)?;
        let owner_source = owner.record.source().and_then(|source| {
            source
                .signature()
                .or_else(|| source.name())
                .map(SourceSpan::source)
        });
        if owner_source != Some(id.expression().source()) {
            return Err(CheckedCallableCatalogBuildError::SourceIdentityMismatch);
        }
        if !row.is_known() {
            return Err(CheckedCallableCatalogBuildError::UnknownEffectRow);
        }
        if self.closure_rows.contains_key(&id) {
            return Err(CheckedCallableCatalogBuildError::DuplicateCallable);
        }
        let source = id.expression().clone();
        if self.closure_source_index.contains_key(&source) {
            return Err(CheckedCallableCatalogBuildError::DuplicateSource);
        }
        self.closure_source_index.insert(source, id.clone());
        self.closure_rows.insert(id, row);
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
    ) -> Result<Arc<CheckedCallableCatalog>, CheckedCallableCatalogBuildError> {
        if self.state != CheckedCatalogBuildState::Validating {
            return Err(CheckedCallableCatalogBuildError::InvalidState);
        }
        self.validate_freeze_indices()?;
        for row in self.closure_rows.values_mut() {
            *row = EffectRow::closed(
                row.resolve(&self.effect_substitution)
                    .map_err(CheckedCallableCatalogBuildError::EffectRow)?,
            );
        }
        let mut records = BTreeMap::new();
        for (id, pending) in std::mem::take(&mut self.pending) {
            if pending.id != id || pending.completion != PendingCallableCompletion::Complete {
                return Err(CheckedCallableCatalogBuildError::MissingInference);
            }
            if let Some(registered) = &self.registered {
                let accepted = registered
                    .record(pending.record.id())
                    .ok_or(CheckedCallableCatalogBuildError::MissingRegisteredRecord)?;
                if !Arc::ptr_eq(accepted, &pending.record) {
                    return Err(CheckedCallableCatalogBuildError::RecordPointerMismatch);
                }
            }
            let effects = finish_effects(pending.contract, pending.inferred)?;
            validate_fact_roles(&pending.record, &pending.execution, &effects)?;
            let interface_digest = interface_digest(
                &pending.record,
                &pending.execution,
                exposed_row(&pending.record, &effects),
            );
            let facts = CheckedCallableFacts {
                id: id.clone(),
                record: pending.record,
                execution: pending.execution,
                effects,
                interface_digest,
            };
            if records.insert(id, facts).is_some() {
                return Err(CheckedCallableCatalogBuildError::DuplicateCallable);
            }
        }
        self.state = CheckedCatalogBuildState::Finished;
        let method_index = freeze_method_index(&mut self.method_index);
        Ok(Arc::new(CheckedCallableCatalog {
            generation: self.generation,
            registered: self.registered,
            records,
            checked_by_candidate: self.checked_by_candidate,
            method_index,
            inaccessible_methods: HashMap::new(),
            closure_rows: self.closure_rows,
            closure_source_index: self.closure_source_index,
            source_index: self.source_index,
        }))
    }

    fn validate_freeze_indices(&self) -> Result<(), CheckedCallableCatalogBuildError> {
        if self.checked_by_candidate.len() != self.pending.len() {
            return Err(CheckedCallableCatalogBuildError::CorruptIndex);
        }
        for (candidate, id) in &self.checked_by_candidate {
            validate_checked_context(&self.generation, id)
                .map_err(|_| CheckedCallableCatalogBuildError::GenerationMismatch)?;
            let pending = self
                .pending
                .get(id)
                .ok_or(CheckedCallableCatalogBuildError::CorruptIndex)?;
            if pending.id != *id || pending.record.id() != candidate {
                return Err(CheckedCallableCatalogBuildError::CorruptIndex);
            }
        }
        for (key, id) in &self.source_index {
            let pending = self
                .pending
                .get(id)
                .ok_or(CheckedCallableCatalogBuildError::CorruptIndex)?;
            let record_source = pending
                .record
                .source()
                .and_then(super::CallableSource::signature)
                .map(SourceSpan::source)
                .ok_or(CheckedCallableCatalogBuildError::SourceIdentityMismatch)?;
            if key.source() != record_source {
                return Err(CheckedCallableCatalogBuildError::SourceIdentityMismatch);
            }
        }
        for (id, pending) in &self.pending {
            let indexed = |category| {
                self.source_index
                    .iter()
                    .any(|(key, indexed)| indexed == id && key.category() == category)
            };
            if pending
                .record
                .source()
                .is_some_and(|source| source.signature().is_some())
                && (!indexed(CheckedCallableSourceCategory::Declaration)
                    || !indexed(CheckedCallableSourceCategory::Signature))
            {
                return Err(CheckedCallableCatalogBuildError::CorruptIndex);
            }
            if pending
                .record
                .source()
                .is_some_and(|source| source.name().is_some())
                && !indexed(CheckedCallableSourceCategory::Name)
            {
                return Err(CheckedCallableCatalogBuildError::CorruptIndex);
            }
            match &pending.contract {
                PendingCallableEffectContract::Body(_) => {
                    if !indexed(CheckedCallableSourceCategory::EffectContract)
                        || !indexed(CheckedCallableSourceCategory::Body)
                    {
                        return Err(CheckedCallableCatalogBuildError::CorruptIndex);
                    }
                }
                PendingCallableEffectContract::BodylessTraitRequirement(_) => {
                    if !indexed(CheckedCallableSourceCategory::EffectContract) {
                        return Err(CheckedCallableCatalogBuildError::CorruptIndex);
                    }
                }
                PendingCallableEffectContract::RecordFixed => {}
            }
        }
        for candidates in self.method_index.values() {
            if candidates.is_empty() {
                return Err(CheckedCallableCatalogBuildError::CorruptIndex);
            }
            for id in candidates {
                let pending = self
                    .pending
                    .get(id)
                    .ok_or(CheckedCallableCatalogBuildError::CorruptIndex)?;
                if pending.record.method_role().is_none()
                    || matches!(pending.record.access(), CallableAccess::TraitImplementation)
                {
                    return Err(CheckedCallableCatalogBuildError::CorruptIndex);
                }
            }
        }
        if self.closure_source_index.len() != self.closure_rows.len() {
            return Err(CheckedCallableCatalogBuildError::CorruptIndex);
        }
        for (closure, row) in &self.closure_rows {
            if !self.pending.contains_key(closure.owner()) || !row.is_known() {
                return Err(CheckedCallableCatalogBuildError::CorruptIndex);
            }
            if self.closure_source_index.get(closure.expression()) != Some(closure) {
                return Err(CheckedCallableCatalogBuildError::CorruptIndex);
            }
            row.resolve(&self.effect_substitution)
                .map_err(CheckedCallableCatalogBuildError::EffectRow)?;
        }
        Ok(())
    }
}

fn freeze_method_index(
    index: &mut HashMap<ReceiverMethodKey, Vec<CheckedCallableId>>,
) -> HashMap<ReceiverMethodKey, Arc<[CheckedCallableId]>> {
    std::mem::take(index)
        .into_iter()
        .map(|(key, mut ids)| {
            ids.sort();
            ids.dedup();
            (key, ids.into())
        })
        .collect()
}

fn freeze_lookup(candidates: &[CheckedCallableId]) -> CheckedMethodLookup {
    let mut candidates = candidates.to_vec();
    candidates.sort();
    candidates.dedup();
    match candidates.as_slice() {
        [] => CheckedMethodLookup::Absent,
        [candidate] => CheckedMethodLookup::Unique(Box::new(candidate.clone())),
        _ => CheckedMethodLookup::Ambiguous(candidates.into()),
    }
}

fn checked_source_keys(
    record: &CallableRecord,
    contract: &PendingCallableEffectContract,
    body_source: Option<&SourceSpan>,
) -> Vec<CheckedCallableSourceKey> {
    let mut keys = Vec::new();
    if let Some(source) = record.source() {
        if let Some(signature) = source.signature() {
            keys.push(CheckedCallableSourceKey::from_span(
                CheckedCallableSourceCategory::Declaration,
                signature,
            ));
            keys.push(CheckedCallableSourceKey::from_span(
                CheckedCallableSourceCategory::Signature,
                signature,
            ));
        }
        if let Some(name) = source.name() {
            keys.push(CheckedCallableSourceKey::from_span(
                CheckedCallableSourceCategory::Name,
                name,
            ));
        }
    }
    match contract {
        PendingCallableEffectContract::Body(contract) => {
            keys.push(CheckedCallableSourceKey::from_span(
                CheckedCallableSourceCategory::EffectContract,
                &contract.source.anchor,
            ));
            if let Some(body_source) = body_source {
                keys.push(CheckedCallableSourceKey::from_span(
                    CheckedCallableSourceCategory::Body,
                    body_source,
                ));
            }
        }
        PendingCallableEffectContract::BodylessTraitRequirement(contract) => {
            keys.push(CheckedCallableSourceKey::from_span(
                CheckedCallableSourceCategory::EffectContract,
                &contract.source.anchor,
            ));
        }
        PendingCallableEffectContract::RecordFixed => {}
    }
    keys
}

fn validate_source_membership(
    record: &CallableRecord,
    keys: &[CheckedCallableSourceKey],
) -> Result<(), CheckedCallableCatalogBuildError> {
    if keys.is_empty() {
        return Ok(());
    }
    let expected = record.source().and_then(|source| {
        source
            .signature()
            .or_else(|| source.name())
            .map(SourceSpan::source)
    });
    if expected.is_some_and(|expected| keys.iter().all(|key| key.source() == expected)) {
        Ok(())
    } else {
        Err(CheckedCallableCatalogBuildError::SourceIdentityMismatch)
    }
}

fn validate_pending_roles(
    record: &CallableRecord,
    execution: &CheckedCallableExecution,
    contract: &PendingCallableEffectContract,
) -> Result<(), CheckedCallableCatalogBuildError> {
    if record
        .method_role()
        .is_some_and(super::CallableMethodRole::is_dispatch_contract)
        != matches!(execution, CheckedCallableExecution::DispatchContract)
    {
        return Err(CheckedCallableCatalogBuildError::InvalidExecutionRole);
    }
    let valid = match contract {
        PendingCallableEffectContract::Body(_) => {
            !matches!(record.schema().effects(), CallableEffectSchema::Fixed(_))
                && !matches!(execution, CheckedCallableExecution::DispatchContract)
        }
        PendingCallableEffectContract::BodylessTraitRequirement(contract) => {
            matches!(execution, CheckedCallableExecution::DispatchContract)
                && matches!(contract.permission(), EffectPermission::Bounded(_))
        }
        PendingCallableEffectContract::RecordFixed => {
            matches!(record.schema().effects(), CallableEffectSchema::Fixed(_))
        }
    };
    if valid {
        Ok(())
    } else {
        Err(CheckedCallableCatalogBuildError::InvalidEffectAuthority)
    }
}

fn finish_effects(
    contract: PendingCallableEffectContract,
    inferred: Option<EffectRow>,
) -> Result<CheckedCallableEffects, CheckedCallableCatalogBuildError> {
    match (contract, inferred) {
        (PendingCallableEffectContract::Body(contract), Some(inferred)) => {
            if inferred.is_known() {
                Ok(CheckedCallableEffects::Body { contract, inferred })
            } else {
                Err(CheckedCallableCatalogBuildError::UnknownEffectRow)
            }
        }
        (PendingCallableEffectContract::BodylessTraitRequirement(contract), None) => {
            Ok(CheckedCallableEffects::BodylessTraitRequirement { contract })
        }
        (PendingCallableEffectContract::RecordFixed, None) => {
            Ok(CheckedCallableEffects::RecordFixed)
        }
        _ => Err(CheckedCallableCatalogBuildError::MissingInference),
    }
}

fn validate_fact_roles(
    record: &CallableRecord,
    execution: &CheckedCallableExecution,
    effects: &CheckedCallableEffects,
) -> Result<(), CheckedCallableCatalogBuildError> {
    if record
        .method_role()
        .is_some_and(super::CallableMethodRole::is_dispatch_contract)
        != matches!(execution, CheckedCallableExecution::DispatchContract)
    {
        return Err(CheckedCallableCatalogBuildError::InvalidExecutionRole);
    }
    let valid_effects = match effects {
        CheckedCallableEffects::Body { .. } => {
            !matches!(record.schema().effects(), CallableEffectSchema::Fixed(_))
                && !matches!(execution, CheckedCallableExecution::DispatchContract)
        }
        CheckedCallableEffects::BodylessTraitRequirement { contract } => {
            matches!(execution, CheckedCallableExecution::DispatchContract)
                && matches!(contract.permission(), EffectPermission::Bounded(_))
        }
        CheckedCallableEffects::RecordFixed => {
            matches!(record.schema().effects(), CallableEffectSchema::Fixed(_))
        }
    };
    if valid_effects {
        Ok(())
    } else {
        Err(CheckedCallableCatalogBuildError::InvalidEffectAuthority)
    }
}

fn exposed_row<'a>(
    record: &'a CallableRecord,
    effects: &'a CheckedCallableEffects,
) -> &'a EffectRow {
    match effects {
        CheckedCallableEffects::Body { contract, inferred } => contract.exposed_or(inferred),
        CheckedCallableEffects::BodylessTraitRequirement { contract } => {
            let EffectPermission::Bounded(row) = contract.permission() else {
                unreachable!("validated bodyless contract is bounded")
            };
            row
        }
        CheckedCallableEffects::RecordFixed => record
            .schema()
            .effects()
            .fixed_row()
            .expect("validated fixed record has fixed row"),
    }
}

fn interface_digest(
    record: &CallableRecord,
    execution: &CheckedCallableExecution,
    exposed: &EffectRow,
) -> CallableInterfaceDigest {
    let mut encoder = super::digest::CanonicalEncoder::default();
    match record.id() {
        CallableCandidateId::Project(declaration) => {
            encoder.tag(0);
            encoder.project_declaration(declaration);
        }
        CallableCandidateId::Environment(declaration) => {
            encoder.tag(1);
            encoder.environment_id(declaration);
        }
        CallableCandidateId::Detached(declaration) => {
            encoder.tag(2);
            encoder.tag(declaration.owner().digest_tag().into());
            encoder.u32(declaration.source_ordinal());
        }
        CallableCandidateId::Standard(declaration) => {
            encoder.tag(3);
            encoder.tag(declaration.owner().digest_tag().into());
            encoder.u32(declaration.catalog_ordinal());
        }
        _ => unreachable!("checked facts admit only record-backed candidates"),
    }
    encoder.bytes(record.schema().semantic_digest().as_bytes());
    encode_access(&mut encoder, record.access());
    encoder.provider(record.provider());
    encoder.option(record.publication_digest().as_ref(), |encoder, digest| {
        encoder.bytes(digest.as_bytes());
    });
    match execution {
        CheckedCallableExecution::DispatchContract => encoder.tag(0),
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame) => encoder.tag(1),
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::StreamFactory {
            item,
            error,
            own_scope_yields,
        }) => {
            encoder.tag(2);
            encoder.bytes(item.semantic_identity_digest().as_bytes());
            encoder.bytes(error.semantic_identity_digest().as_bytes());
            encoder.u32(*own_scope_yields);
        }
    }
    encode_row(&mut encoder, exposed);
    CallableInterfaceDigest(encoder.finish(b"arcweft.callable-interface.v1\0"))
}

fn encode_access(encoder: &mut super::digest::CanonicalEncoder, access: &CallableAccess) {
    match access {
        CallableAccess::Direct {
            declaration_visibility,
        } => {
            encoder.tag(0);
            encode_visibility(encoder, *declaration_visibility);
        }
        CallableAccess::TraitRequirement {
            trait_declaration,
            trait_visibility,
        } => {
            encoder.tag(1);
            encoder.string(trait_declaration.package().as_str());
            encoder.string(&trait_declaration.module().to_string());
            encoder.string(trait_declaration.name().as_str());
            encode_visibility(encoder, *trait_visibility);
        }
        CallableAccess::TraitImplementation => encoder.tag(2),
        CallableAccess::InherentMethod { owner_module } => {
            encoder.tag(3);
            encoder.string(&owner_module.to_string());
        }
        CallableAccess::Environment => encoder.tag(4),
        CallableAccess::Standard => encoder.tag(5),
        CallableAccess::Detached => encoder.tag(6),
        CallableAccess::Structural => encoder.tag(7),
    }
}

fn encode_visibility(
    encoder: &mut super::digest::CanonicalEncoder,
    visibility: Option<arcweft_lang_syntax::ast::common::Visibility>,
) {
    encoder.tag(match visibility {
        None => 0,
        Some(arcweft_lang_syntax::ast::common::Visibility::Public) => 1,
        Some(arcweft_lang_syntax::ast::common::Visibility::Crate) => 2,
        Some(arcweft_lang_syntax::ast::common::Visibility::Super) => 3,
    });
}

fn encode_row(encoder: &mut super::digest::CanonicalEncoder, row: &EffectRow) {
    encoder.usize(row.concrete().len());
    for effect in row.concrete().iter() {
        encoder.string(effect.as_str());
    }
    match row.tail() {
        EffectRowTail::Closed => encoder.tag(0),
        EffectRowTail::Variable(variable) => {
            encoder.tag(1);
            encoder.u32(variable.index());
        }
        EffectRowTail::Unknown => encoder.tag(2),
    }
}
