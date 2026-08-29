//! Final checked statement payloads and their read-only semantic projections.

use arcweft_id::{AcceptedUnsafeAuditSemanticId, LocaleTag, UnsafeAuditId};
use arcweft_lang_hir::symbol::{CallableDeclarationDigest, ImplMethodDeclarationId};
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;

use crate::{
    effects::EffectSet,
    semantic_coordinate::{CheckedControlTransferTarget, StableCheckedDialogueMarkCoordinate},
    types::TypeKind,
};

use super::{CheckedEvaluatedEffect, CheckedFieldSelection, CheckedProjectNominal};
use crate::final_analysis::statement_effects::CompletedStatementEffectFold;
use arcweft_lang_hir::{identity::LocalId, project::HirRuntimeIteratorWitnessMethodRole};

/// Built-in iteration families whose runtime behavior is language-owned.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedIteratorFamily {
    Range,
    Seq,
    Stream,
    Vec,
    Array,
    Slice,
}

/// Generation-bound identity of the selected trait authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTraitIdentity {
    Project(arcweft_lang_hir::identity::ItemId),
    StandardIterator,
    StandardIntoIterator,
}

/// Generation-bound trait conformance used by iteration lowering.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTraitConformance {
    implementation: arcweft_lang_hir::identity::ItemId,
    trait_identity: CheckedTraitIdentity,
    method: u16,
    declaration: Box<ImplMethodDeclarationId>,
}

impl CheckedTraitConformance {
    pub(crate) fn new(
        implementation: arcweft_lang_hir::identity::ItemId,
        trait_identity: CheckedTraitIdentity,
        method: u16,
        declaration: ImplMethodDeclarationId,
    ) -> Self {
        Self {
            implementation,
            trait_identity,
            method,
            declaration: Box::new(declaration),
        }
    }

    pub const fn implementation(&self) -> arcweft_lang_hir::identity::ItemId {
        self.implementation
    }

    pub const fn trait_identity(&self) -> &CheckedTraitIdentity {
        &self.trait_identity
    }

    pub const fn method(&self) -> u16 {
        self.method
    }

    pub const fn declaration(&self) -> &ImplMethodDeclarationId {
        &self.declaration
    }
}

/// Checked iteration dispatch for one final-HIR `for` statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedIteration {
    Builtin {
        family: CheckedIteratorFamily,
        item: TypeKind,
    },
    Witness {
        source: TypeKind,
        item: TypeKind,
        into_iter: TypeKind,
        into_iterator: CheckedTraitConformance,
        iterator: CheckedTraitConformance,
    },
    IteratorWitness {
        source: TypeKind,
        item: TypeKind,
        iterator: CheckedTraitConformance,
    },
}

impl CheckedIteration {
    /// Iterates the exact checked method rows required by this witness.
    pub fn witness_methods(
        &self,
    ) -> impl Iterator<
        Item = (
            HirRuntimeIteratorWitnessMethodRole,
            &CheckedTraitConformance,
            &TypeKind,
        ),
    > + '_ {
        let rows = match self {
            Self::Builtin { .. } => [None, None],
            Self::Witness {
                source,
                into_iter,
                into_iterator,
                iterator,
                ..
            } => [
                Some((
                    HirRuntimeIteratorWitnessMethodRole::IntoIterator,
                    into_iterator,
                    source,
                )),
                Some((
                    HirRuntimeIteratorWitnessMethodRole::IteratorNext,
                    iterator,
                    into_iter,
                )),
            ],
            Self::IteratorWitness {
                source, iterator, ..
            } => [
                Some((
                    HirRuntimeIteratorWitnessMethodRole::IteratorNext,
                    iterator,
                    source,
                )),
                None,
            ],
        };
        rows.into_iter().flatten()
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Builtin { item, .. } => visitor(item),
            Self::Witness {
                source,
                item,
                into_iter,
                ..
            } => {
                visitor(source)?;
                visitor(item)?;
                visitor(into_iter)
            }
            Self::IteratorWitness { source, item, .. } => {
                visitor(source)?;
                visitor(item)
            }
        }
    }
}

/// Final assertion disposition after proof/debug policy admission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedAssertionDisposition {
    /// Awaiting compile-time verifier admission. This never enters runtime lowering.
    PendingProof,
    Discharged,
    Runtime(crate::assertion::AssertionRuntimePolicy),
    OmittedDebug,
}

/// Closed writable place admitted for one final-HIR assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAssignmentPlace {
    local: LocalId,
    nominal: CheckedProjectNominal,
    field: CheckedFieldSelection,
    field_type: TypeKind,
}

impl CheckedAssignmentPlace {
    pub(crate) fn try_new(
        local: LocalId,
        nominal: CheckedProjectNominal,
        field: CheckedFieldSelection,
        field_type: TypeKind,
    ) -> Option<Self> {
        if field.owner_type() != nominal.identity()
            || field.runtime_field().is_none()
            || field.field_type() != field_type.semantic_identity_digest()
        {
            return None;
        }
        Some(Self {
            local,
            nominal,
            field,
            field_type,
        })
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn nominal(&self) -> &CheckedProjectNominal {
        &self.nominal
    }

    pub const fn field(&self) -> &CheckedFieldSelection {
        &self.field
    }

    pub const fn runtime_field(&self) -> Option<arcweft_core::value::RuntimeRecordFieldId> {
        self.field.runtime_field()
    }

    pub const fn field_type(&self) -> &TypeKind {
        &self.field_type
    }
}

/// Complete semantic assignment fact for one final-HIR statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAssignment {
    place: CheckedAssignmentPlace,
    value_type: TypeKind,
}

impl CheckedAssignment {
    pub(crate) const fn new(place: CheckedAssignmentPlace, value_type: TypeKind) -> Self {
        Self { place, value_type }
    }

    pub const fn place(&self) -> &CheckedAssignmentPlace {
        &self.place
    }

    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}

/// Complete semantic disposition for one suspension statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSuspensionStatement {
    Wait,
}

/// Non-child meaning of one accepted trigger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTrigger {
    kind: CheckedTriggerKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckedTriggerKind {
    Input,
    Event,
    Signal,
    Timeout,
    Mark(StableCheckedDialogueMarkCoordinate),
    Select,
    Task,
    Scope,
    Expression,
}

/// Borrowed read-only projection of an accepted trigger fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedTriggerView<'a> {
    Input,
    Event,
    Signal,
    Timeout,
    Mark(&'a StableCheckedDialogueMarkCoordinate),
    Select,
    Task,
    Scope,
    Expression,
}

impl CheckedTrigger {
    pub(crate) const fn input() -> Self {
        Self {
            kind: CheckedTriggerKind::Input,
        }
    }

    pub(crate) const fn event() -> Self {
        Self {
            kind: CheckedTriggerKind::Event,
        }
    }

    pub(crate) const fn signal() -> Self {
        Self {
            kind: CheckedTriggerKind::Signal,
        }
    }

    pub(crate) const fn timeout() -> Self {
        Self {
            kind: CheckedTriggerKind::Timeout,
        }
    }

    pub(crate) const fn mark(coordinate: StableCheckedDialogueMarkCoordinate) -> Self {
        Self {
            kind: CheckedTriggerKind::Mark(coordinate),
        }
    }

    pub(crate) const fn select() -> Self {
        Self {
            kind: CheckedTriggerKind::Select,
        }
    }

    pub(crate) const fn task() -> Self {
        Self {
            kind: CheckedTriggerKind::Task,
        }
    }

    pub(crate) const fn scope() -> Self {
        Self {
            kind: CheckedTriggerKind::Scope,
        }
    }

    pub(crate) const fn expression() -> Self {
        Self {
            kind: CheckedTriggerKind::Expression,
        }
    }

    pub const fn view(&self) -> CheckedTriggerView<'_> {
        match &self.kind {
            CheckedTriggerKind::Input => CheckedTriggerView::Input,
            CheckedTriggerKind::Event => CheckedTriggerView::Event,
            CheckedTriggerKind::Signal => CheckedTriggerView::Signal,
            CheckedTriggerKind::Timeout => CheckedTriggerView::Timeout,
            CheckedTriggerKind::Mark(coordinate) => CheckedTriggerView::Mark(coordinate),
            CheckedTriggerKind::Select => CheckedTriggerView::Select,
            CheckedTriggerKind::Task => CheckedTriggerView::Task,
            CheckedTriggerKind::Scope => CheckedTriggerView::Scope,
            CheckedTriggerKind::Expression => CheckedTriggerView::Expression,
        }
    }

    #[allow(
        dead_code,
        reason = "consumed by the version-one statement transcript cut"
    )]
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match &self.kind {
            CheckedTriggerKind::Input => 0,
            CheckedTriggerKind::Event => 1,
            CheckedTriggerKind::Signal => 2,
            CheckedTriggerKind::Timeout => 3,
            CheckedTriggerKind::Mark(_) => 4,
            CheckedTriggerKind::Select => 5,
            CheckedTriggerKind::Task => 6,
            CheckedTriggerKind::Scope => 7,
            CheckedTriggerKind::Expression => 8,
        }
    }
}

/// Source-ordered, non-child meaning of one Select branch head.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedSelectBranchHead {
    Bind,
    Frame,
    Event,
}

impl CheckedSelectBranchHead {
    #[allow(
        dead_code,
        reason = "consumed by the version-one statement transcript cut"
    )]
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Bind => 0,
            Self::Frame => 1,
            Self::Event => 2,
        }
    }
}

/// Non-child meaning of one accepted Select statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedSelectStatement {
    kind: CheckedSelectStatementKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckedSelectStatementKind {
    Operand,
    Branches(Box<[CheckedSelectBranchHead]>),
}

/// Borrowed read-only projection of one accepted Select statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedSelectStatementView<'a> {
    Operand,
    Branches(&'a [CheckedSelectBranchHead]),
}

impl CheckedSelectStatement {
    pub(crate) const fn operand() -> Self {
        Self {
            kind: CheckedSelectStatementKind::Operand,
        }
    }

    pub(crate) const fn branches(branches: Box<[CheckedSelectBranchHead]>) -> Self {
        Self {
            kind: CheckedSelectStatementKind::Branches(branches),
        }
    }

    pub const fn view(&self) -> CheckedSelectStatementView<'_> {
        match &self.kind {
            CheckedSelectStatementKind::Operand => CheckedSelectStatementView::Operand,
            CheckedSelectStatementKind::Branches(branches) => {
                CheckedSelectStatementView::Branches(branches)
            }
        }
    }

    #[allow(
        dead_code,
        reason = "consumed by the version-one statement transcript cut"
    )]
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match &self.kind {
            CheckedSelectStatementKind::Operand => 0,
            CheckedSelectStatementKind::Branches(_) => 1,
        }
    }
}

/// Accepted unsafe-audit identity and documentation disposition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedUnsafeAudit {
    id: UnsafeAuditId,
    has_safety_doc: bool,
}

impl CheckedUnsafeAudit {
    pub(crate) const fn new(id: UnsafeAuditId, has_safety_doc: bool) -> Self {
        Self { id, has_safety_doc }
    }

    pub const fn id(&self) -> &UnsafeAuditId {
        &self.id
    }

    pub const fn has_safety_doc(&self) -> bool {
        self.has_safety_doc
    }

    pub fn semantic_id(&self) -> AcceptedUnsafeAuditSemanticId {
        self.id.semantic_id()
    }
}

/// Semantic presence of a name on a Scope statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedScopeIdentity {
    Anonymous,
    Named,
}

/// Accepted Flow declaration selected for one Include statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedIncludeFlowTarget {
    declaration: CallableDeclarationDigest,
}

impl CheckedIncludeFlowTarget {
    pub(crate) const fn new(declaration: CallableDeclarationDigest) -> Self {
        Self { declaration }
    }

    pub const fn declaration(&self) -> CallableDeclarationDigest {
        self.declaration
    }
}

/// Complete non-child semantic payload for one accepted statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedStatementPayload {
    Structural,
    Assignment(Box<CheckedAssignment>),
    Assertion(CheckedAssertionDisposition),
    Defer(DeferOutcome),
    EvaluatedEffect(Box<CheckedEvaluatedEffect>),
    Iteration(Box<CheckedIteration>),
    ControlTransfer(CheckedControlTransferTarget),
    Trigger(CheckedTrigger),
    UnsafeAudit(CheckedUnsafeAudit),
    Select(CheckedSelectStatement),
    SourceLocale(LocaleTag),
    Scope(CheckedScopeIdentity),
    Include(CheckedIncludeFlowTarget),
    Suspension(Box<CheckedSuspensionStatement>),
    Yield,
}

impl CheckedStatementPayload {
    #[allow(
        dead_code,
        reason = "consumed by the version-one statement transcript cut"
    )]
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Structural => 0,
            Self::Assignment(_) => 1,
            Self::Assertion(_) => 2,
            Self::Defer(_) => 3,
            Self::EvaluatedEffect(_) => 4,
            Self::Iteration(_) => 5,
            Self::ControlTransfer(_) => 6,
            Self::Trigger(_) => 7,
            Self::UnsafeAudit(_) => 8,
            Self::Select(_) => 9,
            Self::SourceLocale(_) => 10,
            Self::Scope(_) => 11,
            Self::Include(_) => 12,
            Self::Suspension(_) => 13,
            Self::Yield => 14,
        }
    }
}

/// Closed checked fact for one live statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStatement {
    effects: EffectSet,
    payload: CheckedStatementPayload,
}

impl CheckedStatement {
    /// Consumes the only completed effect fold accepted for this payload.
    pub(crate) fn new(
        payload: CheckedStatementPayload,
        fold: CompletedStatementEffectFold,
    ) -> Option<Self> {
        fold.into_effects(&payload)
            .map(|effects| Self { effects, payload })
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn payload(&self) -> &CheckedStatementPayload {
        &self.payload
    }
}
