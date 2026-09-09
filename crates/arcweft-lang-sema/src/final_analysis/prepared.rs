//! Private, consumable semantic facts that require the project-wide C2 seal.

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    identity::{ExprId, ItemId, LocalId, PatternId},
    leaf::HirName,
    symbol::CallableDeclarationKey,
};

use crate::{
    callable::ContentCallableIdentity,
    checked_text_proxy::CheckedCompileTimeScalar,
    effects::EffectSet,
    env::nominal::AcceptedEnvironmentRecord,
    types::{SemanticTypeDigest, TypeKind},
};

pub(crate) use super::model::{PreparedVariantCaseSeed, PreparedVariantOwnerSeed};
use super::{
    CheckedExpression, CheckedExpressionResolution, CheckedPattern, CheckedProjectNominal,
    CheckedTryCarrier, CheckedTypeSelection,
};

#[path = "prepared/evaluated_effect.rs"]
mod evaluated_effect;
pub(crate) use evaluated_effect::PreparedEvaluatedEffect;
#[path = "prepared/dialogue.rs"]
mod dialogue;
pub(crate) use dialogue::{
    PreparedContentApplication, PreparedContentEmission, PreparedDialogueApplication,
    PreparedDialogueEffectPlan, PreparedDialogueEffectSite,
};
#[path = "prepared/statement.rs"]
mod statement;
pub(crate) use statement::{
    PreparedAssignmentStatement, PreparedEventScrutineeProof, PreparedIncludeFlowProof,
    PreparedSelectBranchHeadProof, PreparedSelectScrutineeProof, PreparedStatementPayload,
    PreparedStatementScrutineeProof, PreparedTriggerScrutineeProof,
};

/// Common checked expression state retained while a projection-dependent row
/// is awaiting the one project-wide seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedTypedExpressionResult {
    ty: TypeKind,
    type_selection: CheckedTypeSelection,
}

impl PreparedTypedExpressionResult {
    pub(crate) const fn new(ty: TypeKind, type_selection: CheckedTypeSelection) -> Self {
        Self { ty, type_selection }
    }

    pub(crate) const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub(crate) const fn type_selection(&self) -> CheckedTypeSelection {
        self.type_selection
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PreparedNonValueExpressionResult {
    ContentEmission(ContentCallableIdentity),
}

impl PreparedNonValueExpressionResult {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedExpressionResult {
    Value(PreparedTypedExpressionResult),
    NonValue(PreparedNonValueExpressionResult),
}

impl PreparedExpressionResult {
    pub(crate) const fn value_type(&self) -> Option<&TypeKind> {
        match self {
            Self::Value(value) => Some(value.ty()),
            Self::NonValue(_) => None,
        }
    }

    pub(crate) const fn type_selection(&self) -> Option<CheckedTypeSelection> {
        match self {
            Self::Value(value) => Some(value.type_selection()),
            Self::NonValue(_) => None,
        }
    }
}

/// Common checked expression state retained while a projection-dependent row
/// is awaiting the one project-wide seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedExpressionShell {
    result: PreparedExpressionResult,
    effects: EffectSet,
}

impl PreparedExpressionShell {
    pub(crate) const fn value(
        ty: TypeKind,
        type_selection: CheckedTypeSelection,
        effects: EffectSet,
    ) -> Self {
        Self {
            result: PreparedExpressionResult::Value(PreparedTypedExpressionResult::new(
                ty,
                type_selection,
            )),
            effects,
        }
    }

    pub(crate) const fn content_emission(
        callable: ContentCallableIdentity,
        effects: EffectSet,
    ) -> Self {
        Self {
            result: PreparedExpressionResult::NonValue(
                PreparedNonValueExpressionResult::ContentEmission(callable),
            ),
            effects,
        }
    }

    pub(crate) const fn result(&self) -> &PreparedExpressionResult {
        &self.result
    }

    pub(crate) const fn value_type(&self) -> Option<&TypeKind> {
        self.result.value_type()
    }

    pub(crate) const fn type_selection(&self) -> Option<CheckedTypeSelection> {
        self.result.type_selection()
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub(crate) fn into_value_parts(self) -> Option<(TypeKind, CheckedTypeSelection, EffectSet)> {
        let PreparedExpressionResult::Value(value) = self.result else {
            return None;
        };
        Some((value.ty, value.type_selection, self.effects))
    }
}

/// One receiver-method callee awaiting the exact checked callable join.
///
/// This carrier never enters the public final model. The call owner consumes
/// it only after overload selection has fixed one accepted callable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedMethodExpression {
    shell: PreparedExpressionShell,
    diagnostic_name: HirName,
}

impl PreparedMethodExpression {
    pub(crate) const fn new(shell: PreparedExpressionShell, diagnostic_name: HirName) -> Self {
        Self {
            shell,
            diagnostic_name,
        }
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) fn into_parts(self) -> (PreparedExpressionShell, HirName) {
        (self.shell, self.diagnostic_name)
    }

    #[must_use]
    pub(crate) fn with_type(self, ty: TypeKind) -> Option<Self> {
        let (_, type_selection, effects) = self.shell.into_value_parts()?;
        Some(Self {
            shell: PreparedExpressionShell::value(ty, type_selection, effects),
            diagnostic_name: self.diagnostic_name,
        })
    }
}

/// Closed family of expression resolutions whose final meaning needs one
/// accepted owner coordinate.  Additional owner-bound expression families can
/// be added here as they acquire the same atomic seal without introducing a
/// parallel prepared carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedOwnerBoundResolution {
    ImplicitCallable(PreparedImplicitCallable),
    ImplicitParameter(PreparedImplicitParameter),
    Pipe(PreparedPipe),
    PipeLeft(PreparedPipeLeft),
    Try(PreparedTry),
}

/// Prepared implicit callable body and its contextual function parts. A body
/// may itself be an owner-bound expression; both forms are closed by the same
/// atomic owner-bound seal after callable and pipe identities are issued.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedImplicitCallableBody {
    Complete(CheckedExpression),
    OwnerBound(Box<PreparedOwnerBoundExpression>),
}

impl From<PreparedOwnerBoundExpression> for PreparedImplicitCallableBody {
    fn from(body: PreparedOwnerBoundExpression) -> Self {
        Self::OwnerBound(Box::new(body))
    }
}

/// Prepared implicit callable and its contextual function parts.  The body
/// remains closed over either a complete checked expression or the owner
/// placeholder marker above until the identity seal supplies its identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedImplicitCallable {
    parameter: TypeKind,
    result: TypeKind,
    body: PreparedImplicitCallableBody,
}

impl PreparedImplicitCallable {
    pub(crate) const fn parameter(&self) -> &TypeKind {
        &self.parameter
    }

    pub(crate) const fn result(&self) -> &TypeKind {
        &self.result
    }

    pub(crate) const fn body(&self) -> &PreparedImplicitCallableBody {
        &self.body
    }
}

/// Prepared partial-application placeholder.  The callable owner is lookup
/// evidence only; the owner-bound seal joins it to an opaque callable identity
/// and a callable-local occurrence ordinal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedImplicitParameter {
    callable: ExprId,
    parameter: TypeKind,
}

impl PreparedImplicitParameter {
    pub(crate) const fn callable(&self) -> ExprId {
        self.callable
    }

    pub(crate) const fn parameter(&self) -> &TypeKind {
        &self.parameter
    }
}

/// Prepared carrier and lexical boundary for one prefix Try expression. The
/// operand remains a normal HIR child edge until the owner-bound seal issues
/// its exact checked evidence; this row retains the carrier and boundary
/// information that cannot be finalized before accepted coordinates and
/// callable identities exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedTry {
    carrier: CheckedTryCarrier,
    boundary: PreparedTryBoundary,
}

impl PreparedTry {
    pub(crate) const fn new(carrier: CheckedTryCarrier, boundary: PreparedTryBoundary) -> Self {
        Self { carrier, boundary }
    }

    pub(crate) const fn carrier(&self) -> &CheckedTryCarrier {
        &self.carrier
    }

    pub(crate) const fn boundary(&self) -> &PreparedTryBoundary {
        &self.boundary
    }
}

/// Lookup-only boundary data retained until the owner-bound phase can issue
/// accepted expression coordinates, implicit callable identities, and
/// declaration-root identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedTryBoundary {
    Infallible,
    CarrierBlock {
        lookup_owner: ExprId,
    },
    ExplicitFunctionSite {
        lookup_owner: ExprId,
        boundary_type: TypeKind,
    },
    ImplicitFunctionSite {
        lookup_owner: ExprId,
        boundary_type: TypeKind,
    },
    Callable {
        declaration: CallableDeclarationKey,
        boundary_type: TypeKind,
    },
}

impl PreparedTryBoundary {
    pub(crate) const fn boundary_type(&self) -> Option<&TypeKind> {
        match self {
            Self::ExplicitFunctionSite { boundary_type, .. }
            | Self::ImplicitFunctionSite { boundary_type, .. }
            | Self::Callable { boundary_type, .. } => Some(boundary_type),
            Self::Infallible | Self::CarrierBlock { .. } => None,
        }
    }
}

/// Prepared once-only pipe binding awaiting its accepted owner coordinate and
/// opaque binding identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedPipe {
    lookup_left: ExprId,
    lookup_right: ExprId,
    left_value_type: TypeKind,
    placeholders: Box<[ExprId]>,
}

impl PreparedPipe {
    pub(crate) fn new(
        lookup_left: ExprId,
        lookup_right: ExprId,
        left_value_type: TypeKind,
        placeholders: impl Into<Box<[ExprId]>>,
    ) -> Self {
        Self {
            lookup_left,
            lookup_right,
            left_value_type,
            placeholders: placeholders.into(),
        }
    }

    pub(crate) const fn lookup_left(&self) -> ExprId {
        self.lookup_left
    }

    pub(crate) const fn lookup_right(&self) -> ExprId {
        self.lookup_right
    }

    pub(crate) const fn left_value_type(&self) -> &TypeKind {
        &self.left_value_type
    }

    pub(crate) const fn placeholders(&self) -> &[ExprId] {
        &self.placeholders
    }
}

/// Prepared `^` occurrence awaiting the identity of its nearest pipe owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedPipeLeft {
    pipe: ExprId,
}

impl PreparedPipeLeft {
    pub(crate) const fn new(pipe: ExprId) -> Self {
        Self { pipe }
    }

    pub(crate) const fn pipe(&self) -> ExprId {
        self.pipe
    }
}

impl PreparedOwnerBoundResolution {
    pub(crate) fn implicit_callable(
        parameter: TypeKind,
        result: TypeKind,
        body: PreparedImplicitCallableBody,
    ) -> Self {
        Self::ImplicitCallable(PreparedImplicitCallable {
            parameter,
            result,
            body,
        })
    }

    pub(crate) fn implicit_parameter(callable: ExprId, parameter: TypeKind) -> Self {
        Self::ImplicitParameter(PreparedImplicitParameter {
            callable,
            parameter,
        })
    }

    pub(crate) fn pipe(
        lookup_left: ExprId,
        lookup_right: ExprId,
        left_value_type: TypeKind,
        placeholders: impl Into<Box<[ExprId]>>,
    ) -> Self {
        Self::Pipe(PreparedPipe::new(
            lookup_left,
            lookup_right,
            left_value_type,
            placeholders,
        ))
    }

    pub(crate) const fn pipe_left(pipe: ExprId) -> Self {
        Self::PipeLeft(PreparedPipeLeft::new(pipe))
    }

    pub(crate) const fn try_expression(
        carrier: CheckedTryCarrier,
        boundary: PreparedTryBoundary,
    ) -> Self {
        Self::Try(PreparedTry::new(carrier, boundary))
    }
}

/// Prepared expression whose resolution is closed only after the accepted
/// owner-bound coordinate phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedOwnerBoundExpression {
    shell: PreparedExpressionShell,
    resolution: PreparedOwnerBoundResolution,
}

impl PreparedOwnerBoundExpression {
    pub(crate) const fn new(
        shell: PreparedExpressionShell,
        resolution: PreparedOwnerBoundResolution,
    ) -> Self {
        Self { shell, resolution }
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) const fn resolution(&self) -> &PreparedOwnerBoundResolution {
        &self.resolution
    }

    pub(crate) const fn value_type(&self) -> Option<&TypeKind> {
        self.shell.value_type()
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        self.shell.effects()
    }

    pub(crate) const fn checked_call_site(
        &self,
        owner: ExprId,
    ) -> Option<crate::callable::CheckedCallSite> {
        match self.resolution() {
            PreparedOwnerBoundResolution::ImplicitCallable(callable) => match callable.body() {
                PreparedImplicitCallableBody::Complete(body) => {
                    body.resolution().checked_call_site(owner)
                }
                PreparedImplicitCallableBody::OwnerBound(_) => None,
            },
            PreparedOwnerBoundResolution::ImplicitParameter(_)
            | PreparedOwnerBoundResolution::Pipe(_)
            | PreparedOwnerBoundResolution::PipeLeft(_)
            | PreparedOwnerBoundResolution::Try(_) => None,
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(ty) = self.value_type() {
            visitor(ty)?;
        }
        match self.resolution() {
            PreparedOwnerBoundResolution::ImplicitCallable(callable) => {
                visitor(callable.parameter())?;
                visitor(callable.result())?;
                match callable.body() {
                    PreparedImplicitCallableBody::Complete(body) => body.visit_types(visitor),
                    PreparedImplicitCallableBody::OwnerBound(body) => body.visit_types(visitor),
                }
            }
            PreparedOwnerBoundResolution::ImplicitParameter(parameter) => {
                visitor(parameter.parameter())
            }
            PreparedOwnerBoundResolution::Pipe(pipe) => {
                visitor(pipe.left_value_type())?;
                Ok(())
            }
            PreparedOwnerBoundResolution::PipeLeft(_) => Ok(()),
            PreparedOwnerBoundResolution::Try(tried) => {
                tried.carrier().visit_types(visitor)?;
                if let Some(boundary_type) = tried.boundary().boundary_type() {
                    visitor(boundary_type)?;
                }
                Ok(())
            }
        }
    }
}

/// Entry identity admitted during expression checking but not yet joined to
/// the checked Entry catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedEntryReference {
    diagnostic_public_id: PublicId,
    lookup_owner: ItemId,
}

impl PreparedEntryReference {
    pub(crate) const fn new(diagnostic_public_id: PublicId, lookup_owner: ItemId) -> Self {
        Self {
            diagnostic_public_id,
            lookup_owner,
        }
    }

    pub(crate) const fn diagnostic_public_id(&self) -> &PublicId {
        &self.diagnostic_public_id
    }

    pub(crate) fn into_parts(self) -> (PublicId, ItemId) {
        (self.diagnostic_public_id, self.lookup_owner)
    }
}

/// One Entry-reference expression awaiting the consuming Entry join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedEntryExpression {
    reference: PreparedEntryReference,
    shell: PreparedExpressionShell,
    value_type: SemanticTypeDigest,
}

impl PreparedEntryExpression {
    pub(crate) fn new(
        reference: PreparedEntryReference,
        type_selection: CheckedTypeSelection,
    ) -> Self {
        let ty = TypeKind::entity_ref(crate::types::EntityKind::Entry);
        let value_type = ty
            .semantic_identity_digest()
            .expect("the Entry entity leaf contains no generic references");
        Self {
            reference,
            shell: PreparedExpressionShell::value(ty, type_selection, EffectSet::new()),
            value_type,
        }
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedEntryReference,
        PreparedExpressionShell,
        SemanticTypeDigest,
    ) {
        (self.reference, self.shell, self.value_type)
    }
}

/// One variant expression awaiting completed type and case identity sealing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantExpression {
    shell: PreparedExpressionShell,
    owner: PreparedVariantOwnerSeed,
    selected_ordinal: u32,
}

impl PreparedVariantExpression {
    pub(crate) fn try_map_types<E: From<super::CheckedVariantOwnerError>>(
        &self,
        map: &mut impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        let owner = self.owner.try_map_types(map)?;
        let shell = PreparedExpressionShell::value(
            owner.ty(),
            self.shell
                .type_selection()
                .expect("variant expressions always carry a value type"),
            self.shell.effects().clone(),
        );
        Ok(Self {
            shell,
            owner,
            selected_ordinal: self.selected_ordinal,
        })
    }

    pub(crate) fn try_new(
        shell: PreparedExpressionShell,
        owner: PreparedVariantOwnerSeed,
        selected_ordinal: u32,
    ) -> Option<Self> {
        if shell.value_type() != Some(&owner.ty()) {
            return None;
        }
        owner
            .cases()
            .get(usize::try_from(selected_ordinal).ok()?)
            .filter(|case| case.ordinal() == selected_ordinal)?;
        Some(Self {
            shell,
            owner,
            selected_ordinal,
        })
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) const fn owner(&self) -> &PreparedVariantOwnerSeed {
        &self.owner
    }

    pub(crate) const fn selected_ordinal(&self) -> u32 {
        self.selected_ordinal
    }

    pub(crate) fn into_parts(self) -> (PreparedExpressionShell, PreparedVariantOwnerSeed, u32) {
        (self.shell, self.owner, self.selected_ordinal)
    }
}

/// One project-field selection awaiting a cached runtime-field coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectFieldExpression {
    shell: PreparedExpressionShell,
    nominal: CheckedProjectNominal,
    declaration_ordinal: u32,
    field_type: TypeKind,
    diagnostic_name: HirName,
}

impl PreparedProjectFieldExpression {
    pub(crate) const fn new(
        shell: PreparedExpressionShell,
        nominal: CheckedProjectNominal,
        declaration_ordinal: u32,
        field_type: TypeKind,
        diagnostic_name: HirName,
    ) -> Self {
        Self {
            shell,
            nominal,
            declaration_ordinal,
            field_type,
            diagnostic_name,
        }
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }
    pub(crate) const fn nominal(&self) -> &CheckedProjectNominal {
        &self.nominal
    }
    pub(crate) const fn field_type(&self) -> &TypeKind {
        &self.field_type
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedExpressionShell,
        CheckedProjectNominal,
        u32,
        TypeKind,
        HirName,
    ) {
        (
            self.shell,
            self.nominal,
            self.declaration_ordinal,
            self.field_type,
            self.diagnostic_name,
        )
    }
}

/// One project-nominal type value awaiting the single project-wide semantic
/// catalog seal.
///
/// A type value is semantic-only: it has the meta-type of its exact nominal,
/// carries no effects, and must not be represented as a checked resolution
/// before the catalog owner has issued its semantic-definition digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectNominalTypeValueExpression {
    shell: PreparedExpressionShell,
    nominal: CheckedProjectNominal,
}

impl PreparedProjectNominalTypeValueExpression {
    pub(crate) fn try_new(nominal: CheckedProjectNominal) -> Option<Self> {
        let nominal_type = nominal.ty();
        if nominal.identity() != nominal_type.semantic_identity_digest().ok()? {
            return None;
        }
        Some(Self {
            shell: PreparedExpressionShell::value(
                TypeKind::MetaType(Box::new(nominal_type)),
                CheckedTypeSelection::Expected,
                EffectSet::new(),
            ),
            nominal,
        })
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) const fn nominal(&self) -> &CheckedProjectNominal {
        &self.nominal
    }

    pub(crate) fn into_parts(self) -> (PreparedExpressionShell, CheckedProjectNominal) {
        (self.shell, self.nominal)
    }
}

/// Analyzer-owned source of one authored record value.
///
/// These generation-local owners remain cloneable while candidate
/// transactions are live. The project-wide draft seal consumes them into
/// issuer-backed stable coordinates before final publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedRecordValueSource {
    Expression(ExprId),
    Local(LocalId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectRecordExpressionField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    field_type: TypeKind,
    source: PreparedRecordValueSource,
}

impl PreparedProjectRecordExpressionField {
    pub(crate) const fn new(
        source_ordinal: u32,
        declaration_ordinal: u32,
        field_type: TypeKind,
        source: PreparedRecordValueSource,
    ) -> Self {
        Self {
            source_ordinal,
            declaration_ordinal,
            field_type,
            source,
        }
    }
    pub(crate) const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }
    pub(crate) const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }
    pub(crate) const fn field_type(&self) -> &TypeKind {
        &self.field_type
    }
    pub(crate) const fn source(&self) -> PreparedRecordValueSource {
        self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectRecordExpression {
    shell: PreparedExpressionShell,
    nominal: CheckedProjectNominal,
    fields: Box<[PreparedProjectRecordExpressionField]>,
}

impl PreparedProjectRecordExpression {
    pub(crate) const fn new(
        shell: PreparedExpressionShell,
        nominal: CheckedProjectNominal,
        fields: Box<[PreparedProjectRecordExpressionField]>,
    ) -> Self {
        Self {
            shell,
            nominal,
            fields,
        }
    }
    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }
    pub(crate) const fn nominal(&self) -> &CheckedProjectNominal {
        &self.nominal
    }
    pub(crate) const fn fields(&self) -> &[PreparedProjectRecordExpressionField] {
        &self.fields
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedExpressionShell,
        CheckedProjectNominal,
        Box<[PreparedProjectRecordExpressionField]>,
    ) {
        (self.shell, self.nominal, self.fields)
    }
}

/// Prepared scalar evidence. The shell owns the exact contextual scalar type;
/// the original fact retains only source-shaped resolution/call authority and
/// any projection-dependent work that still has to pass through the C2 seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCompileTimeScalarExpression {
    shell: PreparedExpressionShell,
    value: CheckedCompileTimeScalar,
    original: PreparedExpressionFact,
}

impl PreparedCompileTimeScalarExpression {
    pub(crate) fn try_new(
        shell: PreparedExpressionShell,
        value: CheckedCompileTimeScalar,
        original: PreparedExpressionFact,
    ) -> Option<Self> {
        if !shell.effects().is_empty()
            || shell.effects() != original.effects()
            || matches!(original, PreparedExpressionFact::CompileTimeScalar(_))
        {
            return None;
        }
        Some(Self {
            shell,
            value,
            original,
        })
    }

    pub(crate) const fn shell(&self) -> &PreparedExpressionShell {
        &self.shell
    }

    pub(crate) const fn value(&self) -> &CheckedCompileTimeScalar {
        &self.value
    }

    pub(crate) const fn original(&self) -> &PreparedExpressionFact {
        &self.original
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedExpressionShell,
        CheckedCompileTimeScalar,
        PreparedExpressionFact,
    ) {
        (self.shell, self.value, self.original)
    }
}

/// Analyzer-owned expression fact. Only `Complete` may enter the published
/// report; every other row is consumed by the private project seal.
/// Each variant owns a heap payload. `Complete` already owns that allocation
/// through `CheckedExpression`; prepared payloads retain independent
/// ownership across evaluation, candidate transactions and final sealing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedExpressionFact {
    Complete(CheckedExpression),
    OwnerBound(Box<PreparedOwnerBoundExpression>),
    CompileTimeScalar(Box<PreparedCompileTimeScalarExpression>),
    DialogueApplication(Box<PreparedDialogueApplication>),
    ContentApplication(Box<PreparedContentApplication>),
    Method(Box<PreparedMethodExpression>),
    Entry(Box<PreparedEntryExpression>),
    Variant(Box<PreparedVariantExpression>),
    ProjectField(Box<PreparedProjectFieldExpression>),
    ProjectRecord(Box<PreparedProjectRecordExpression>),
    ProjectNominalTypeValue(Box<PreparedProjectNominalTypeValueExpression>),
}

impl From<CheckedExpression> for PreparedExpressionFact {
    fn from(value: CheckedExpression) -> Self {
        Self::Complete(value)
    }
}

impl From<PreparedOwnerBoundExpression> for PreparedExpressionFact {
    fn from(value: PreparedOwnerBoundExpression) -> Self {
        Self::OwnerBound(Box::new(value))
    }
}

impl From<PreparedCompileTimeScalarExpression> for PreparedExpressionFact {
    fn from(value: PreparedCompileTimeScalarExpression) -> Self {
        Self::CompileTimeScalar(Box::new(value))
    }
}

impl From<PreparedDialogueApplication> for PreparedExpressionFact {
    fn from(value: PreparedDialogueApplication) -> Self {
        Self::DialogueApplication(Box::new(value))
    }
}

impl From<PreparedContentApplication> for PreparedExpressionFact {
    fn from(value: PreparedContentApplication) -> Self {
        Self::ContentApplication(Box::new(value))
    }
}

impl From<PreparedMethodExpression> for PreparedExpressionFact {
    fn from(value: PreparedMethodExpression) -> Self {
        Self::Method(Box::new(value))
    }
}

impl From<PreparedEntryExpression> for PreparedExpressionFact {
    fn from(value: PreparedEntryExpression) -> Self {
        Self::Entry(Box::new(value))
    }
}

impl From<PreparedVariantExpression> for PreparedExpressionFact {
    fn from(value: PreparedVariantExpression) -> Self {
        Self::Variant(Box::new(value))
    }
}

impl From<PreparedProjectFieldExpression> for PreparedExpressionFact {
    fn from(value: PreparedProjectFieldExpression) -> Self {
        Self::ProjectField(Box::new(value))
    }
}

impl From<PreparedProjectRecordExpression> for PreparedExpressionFact {
    fn from(value: PreparedProjectRecordExpression) -> Self {
        Self::ProjectRecord(Box::new(value))
    }
}

impl From<PreparedProjectNominalTypeValueExpression> for PreparedExpressionFact {
    fn from(value: PreparedProjectNominalTypeValueExpression) -> Self {
        Self::ProjectNominalTypeValue(Box::new(value))
    }
}

impl PreparedExpressionFact {
    /// Returns the execution-local use represented directly by this prepared
    /// expression fact.
    ///
    /// Only a completed fact with a direct `Value(Local)` resolution is an
    /// execution use. In particular, this does not unwrap
    /// `CompileTimeScalar`'s retained source fact.
    pub(crate) const fn execution_local_use(&self) -> Option<LocalId> {
        match self {
            Self::Complete(value) => value.execution_local_use(),
            Self::OwnerBound(_)
            | Self::CompileTimeScalar(_)
            | Self::DialogueApplication(_)
            | Self::ContentApplication(_)
            | Self::Method(_)
            | Self::Entry(_)
            | Self::Variant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_)
            | Self::ProjectNominalTypeValue(_) => None,
        }
    }

    /// Runtime value type contributed by this prepared expression, if any.
    /// Semantic-only scalar/type/callee facts and non-value content carriers
    /// remain available to semantic sealing but cannot seed runtime nominal
    /// projection.
    pub(crate) const fn runtime_value_type(&self) -> Option<&TypeKind> {
        match self {
            Self::OwnerBound(value) => value.shell().value_type(),
            Self::CompileTimeScalar(_) | Self::DialogueApplication(_) | Self::Method(_) => None,
            Self::ContentApplication(value) => match value.emission() {
                PreparedContentEmission::ContentResult => value.shell().value_type(),
                PreparedContentEmission::ObjectSpan(_)
                | PreparedContentEmission::LanguageCallable(_) => None,
            },
            Self::Complete(value)
                if matches!(
                    value.execution_plan().value(),
                    super::CheckedRuntimeValueDisposition::Omit
                ) =>
            {
                None
            }
            Self::Complete(value) => value.value_type(),
            Self::Entry(value) => value.shell().value_type(),
            Self::Variant(value) => value.shell().value_type(),
            Self::ProjectField(value) => value.shell().value_type(),
            Self::ProjectRecord(value) => value.shell().value_type(),
            Self::ProjectNominalTypeValue(_) => None,
        }
    }

    pub(crate) const fn value_type(&self) -> Option<&TypeKind> {
        match self {
            Self::Complete(value) => value.value_type(),
            Self::OwnerBound(value) => value.shell().value_type(),
            Self::CompileTimeScalar(value) => value.shell().value_type(),
            Self::DialogueApplication(value) => value.shell().value_type(),
            Self::ContentApplication(value) => value.shell().value_type(),
            Self::Method(value) => value.shell().value_type(),
            Self::Entry(value) => value.shell().value_type(),
            Self::Variant(value) => value.shell().value_type(),
            Self::ProjectField(value) => value.shell().value_type(),
            Self::ProjectRecord(value) => value.shell().value_type(),
            Self::ProjectNominalTypeValue(value) => value.shell().value_type(),
        }
    }

    pub(crate) const fn type_selection(&self) -> Option<CheckedTypeSelection> {
        match self {
            Self::Complete(value) => value.type_selection(),
            Self::OwnerBound(value) => value.shell().type_selection(),
            Self::CompileTimeScalar(value) => value.shell().type_selection(),
            Self::DialogueApplication(value) => value.shell().type_selection(),
            Self::ContentApplication(value) => value.shell().type_selection(),
            Self::Method(value) => value.shell().type_selection(),
            Self::Entry(value) => value.shell().type_selection(),
            Self::Variant(value) => value.shell().type_selection(),
            Self::ProjectField(value) => value.shell().type_selection(),
            Self::ProjectRecord(value) => value.shell().type_selection(),
            Self::ProjectNominalTypeValue(value) => value.shell().type_selection(),
        }
    }

    pub(crate) fn required_type_selection(
        &self,
        owner: ExprId,
    ) -> Result<CheckedTypeSelection, super::FinalSemanticAnalysisError> {
        self.type_selection()
            .ok_or(super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        match self {
            Self::Complete(value) => value.effects(),
            Self::OwnerBound(value) => value.shell().effects(),
            Self::CompileTimeScalar(value) => value.shell().effects(),
            Self::DialogueApplication(value) => value.shell().effects(),
            Self::ContentApplication(value) => value.shell().effects(),
            Self::Method(value) => value.shell().effects(),
            Self::Entry(value) => value.shell().effects(),
            Self::Variant(value) => value.shell().effects(),
            Self::ProjectField(value) => value.shell().effects(),
            Self::ProjectRecord(value) => value.shell().effects(),
            Self::ProjectNominalTypeValue(value) => value.shell().effects(),
        }
    }

    pub(crate) const fn complete(&self) -> Option<&CheckedExpression> {
        match self {
            Self::Complete(value) => Some(value),
            Self::OwnerBound(_) => None,
            Self::CompileTimeScalar(_) => None,
            Self::Method(_)
            | Self::DialogueApplication(_)
            | Self::ContentApplication(_)
            | Self::Entry(_)
            | Self::Variant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_)
            | Self::ProjectNominalTypeValue(_) => None,
        }
    }

    /// Returns the dedicated pre-C2 project-nominal type-value carrier.
    ///
    /// This accessor intentionally does not expose it through
    /// [`Self::checked_resolution`]: the final `TypeValue` resolution exists
    /// only after the nominal semantic catalog has issued its opaque digest.
    pub(crate) const fn project_nominal_type_value(
        &self,
    ) -> Option<&PreparedProjectNominalTypeValueExpression> {
        match self {
            Self::ProjectNominalTypeValue(value) => Some(value),
            Self::Complete(_)
            | Self::OwnerBound(_)
            | Self::CompileTimeScalar(_)
            | Self::DialogueApplication(_)
            | Self::ContentApplication(_)
            | Self::Method(_)
            | Self::Entry(_)
            | Self::Variant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_) => None,
        }
    }

    pub(crate) fn into_complete(self) -> Result<CheckedExpression, Self> {
        match self {
            Self::Complete(value) => Ok(value),
            Self::OwnerBound(value) => Err(Self::OwnerBound(value)),
            Self::CompileTimeScalar(value) => {
                let (shell, scalar, original) = value.into_parts();
                let Some((ty, type_selection, effects)) = shell.clone().into_value_parts() else {
                    return Err(Self::from(PreparedCompileTimeScalarExpression {
                        shell,
                        value: scalar,
                        original,
                    }));
                };
                match original.into_complete() {
                    Ok(complete) => {
                        let resolution = CheckedExpressionResolution::CompileTimeScalar(
                            super::CheckedCompileTimeScalarExpression::new(
                                scalar,
                                complete.resolution().clone(),
                            ),
                        );
                        Ok(CheckedExpression::value(
                            ty,
                            type_selection,
                            effects,
                            resolution,
                        ))
                    }
                    Err(original) => Err(Self::from(PreparedCompileTimeScalarExpression {
                        shell,
                        value: scalar,
                        original,
                    })),
                }
            }
            Self::DialogueApplication(value) => Err(Self::DialogueApplication(value)),
            Self::ContentApplication(value) => Err(Self::ContentApplication(value)),
            Self::Method(value) => Err(Self::Method(value)),
            Self::Entry(value) => Err(Self::Entry(value)),
            Self::Variant(value) => Err(Self::Variant(value)),
            Self::ProjectField(value) => Err(Self::ProjectField(value)),
            Self::ProjectRecord(value) => Err(Self::ProjectRecord(value)),
            Self::ProjectNominalTypeValue(value) => Err(Self::ProjectNominalTypeValue(value)),
        }
    }

    pub(crate) const fn checked_resolution(&self) -> Option<&CheckedExpressionResolution> {
        match self {
            Self::Complete(value) => Some(value.resolution()),
            Self::OwnerBound(_) => None,
            Self::CompileTimeScalar(value) => value.original().checked_resolution(),
            Self::DialogueApplication(_)
            | Self::ContentApplication(_)
            | Self::Method(_)
            | Self::Entry(_)
            | Self::Variant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_)
            | Self::ProjectNominalTypeValue(_) => None,
        }
    }

    pub(crate) fn reusable_for_parametric_expectation(&self, expected: &TypeKind) -> bool {
        let Some(value_type) = self.value_type() else {
            return false;
        };
        self.type_selection() != Some(CheckedTypeSelection::Expected)
            || value_type.semantic_identity_digest() == expected.semantic_identity_digest()
    }

    pub(crate) const fn checked_call_site(
        &self,
        owner: ExprId,
    ) -> Option<crate::callable::CheckedCallSite> {
        match self.checked_resolution() {
            Some(resolution) => resolution.checked_call_site(owner),
            None => match self {
                Self::OwnerBound(value) => value.checked_call_site(owner),
                Self::CompileTimeScalar(value) => value.original().checked_call_site(owner),
                Self::DialogueApplication(_) => Some(
                    crate::callable::CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                    },
                ),
                Self::ContentApplication(_) => Some(
                    crate::callable::CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    },
                ),
                Self::Complete(_)
                | Self::Method(_)
                | Self::Entry(_)
                | Self::Variant(_)
                | Self::ProjectField(_)
                | Self::ProjectRecord(_)
                | Self::ProjectNominalTypeValue(_) => None,
            },
        }
    }

    pub(crate) const fn selected_postfix_candidate(&self) -> Option<ExprId> {
        match self {
            Self::Complete(value) => value.selected_postfix_candidate(),
            Self::OwnerBound(_) => None,
            Self::CompileTimeScalar(value) => value.original().selected_postfix_candidate(),
            Self::DialogueApplication(_)
            | Self::ContentApplication(_)
            | Self::Method(_)
            | Self::Entry(_)
            | Self::Variant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_)
            | Self::ProjectNominalTypeValue(_) => None,
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Complete(value) => value.visit_types(visitor),
            Self::OwnerBound(value) => value.visit_types(visitor),
            Self::CompileTimeScalar(value) => {
                if let Some(ty) = value.shell().value_type() {
                    visitor(ty)?;
                }
                value.original().visit_types(visitor)
            }
            Self::DialogueApplication(value) => {
                value.target().visit_types(visitor)?;
                if let Some(patch) = value.application_patch() {
                    patch.visit_types(visitor)?;
                }
                visitor(value.line_result())
            }
            Self::ContentApplication(value) => value.shell().value_type().map_or(Ok(()), visitor),
            Self::Method(value) => value.shell().value_type().map_or(Ok(()), visitor),
            Self::Entry(value) => value.shell().value_type().map_or(Ok(()), visitor),
            Self::Variant(value) => {
                if let Some(ty) = value.shell().value_type() {
                    visitor(ty)?;
                }
                value.owner().visit_types(visitor)
            }
            Self::ProjectField(value) => {
                if let Some(ty) = value.shell().value_type() {
                    visitor(ty)?;
                }
                value.nominal().visit_types(visitor)?;
                visitor(value.field_type())
            }
            Self::ProjectRecord(value) => {
                if let Some(ty) = value.shell().value_type() {
                    visitor(ty)?;
                }
                value.nominal().visit_types(visitor)?;
                for field in value.fields() {
                    visitor(field.field_type())?;
                }
                Ok(())
            }
            Self::ProjectNominalTypeValue(value) => {
                if let Some(ty) = value.shell().value_type() {
                    visitor(ty)?;
                }
                value.nominal().visit_types(visitor)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantPattern {
    ty: TypeKind,
    owner: PreparedVariantOwnerSeed,
    selected_ordinal: u32,
}

impl PreparedVariantPattern {
    pub(crate) fn try_new(
        ty: TypeKind,
        owner: PreparedVariantOwnerSeed,
        selected_ordinal: u32,
    ) -> Option<Self> {
        if ty != owner.ty() {
            return None;
        }
        owner
            .cases()
            .get(usize::try_from(selected_ordinal).ok()?)
            .filter(|case| case.ordinal() == selected_ordinal)?;
        Some(Self {
            ty,
            owner,
            selected_ordinal,
        })
    }
    pub(crate) const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub(crate) const fn owner(&self) -> &PreparedVariantOwnerSeed {
        &self.owner
    }
    pub(crate) fn into_parts(self) -> (TypeKind, PreparedVariantOwnerSeed, u32) {
        (self.ty, self.owner, self.selected_ordinal)
    }
}

/// Analyzer-owned source of one authored record-pattern field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedRecordPatternSource {
    Pattern(PatternId),
    Binding(LocalId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedRecordPatternRest {
    Absent,
    Ignore,
    Binding(LocalId),
}

/// A declaration-ordered coordinate in the prepared record owner's schema.
/// Stable field identities are issued only by the consuming final seal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedRecordPatternFieldCoordinate(u32);

impl PreparedRecordPatternFieldCoordinate {
    pub(crate) const fn new(declaration_ordinal: u32) -> Self {
        Self(declaration_ordinal)
    }
    pub(crate) const fn declaration_ordinal(self) -> u32 {
        self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedRecordPatternField {
    source_ordinal: u32,
    coordinate: PreparedRecordPatternFieldCoordinate,
    field_type: TypeKind,
    source: PreparedRecordPatternSource,
}

impl PreparedRecordPatternField {
    pub(crate) const fn new(
        source_ordinal: u32,
        coordinate: PreparedRecordPatternFieldCoordinate,
        field_type: TypeKind,
        source: PreparedRecordPatternSource,
    ) -> Self {
        Self {
            source_ordinal,
            coordinate: coordinate,
            field_type,
            source,
        }
    }
    pub(crate) const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }
    pub(crate) const fn coordinate(&self) -> PreparedRecordPatternFieldCoordinate {
        self.coordinate
    }
    pub(crate) const fn field_type(&self) -> &TypeKind {
        &self.field_type
    }
    pub(crate) const fn source(&self) -> PreparedRecordPatternSource {
        self.source
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        u32,
        PreparedRecordPatternFieldCoordinate,
        TypeKind,
        PreparedRecordPatternSource,
    ) {
        (
            self.source_ordinal,
            self.coordinate,
            self.field_type,
            self.source,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedRecordPatternOwner {
    Project(CheckedProjectNominal),
    Environment {
        record: AcceptedEnvironmentRecord,
    },
    VariantPayload {
        payload: crate::types::VariantPayloadType,
        field_count: u32,
    },
}

impl PreparedRecordPatternOwner {
    pub(crate) fn semantic_type(
        &self,
    ) -> Result<SemanticTypeDigest, crate::types::GenericScopeError> {
        match self {
            Self::Project(nominal) => Ok(nominal.identity()),
            Self::Environment { record } => Ok(record.semantic_type()),
            Self::VariantPayload { payload, .. } => {
                TypeKind::VariantPayload(Box::new(payload.clone())).semantic_identity_digest()
            }
        }
    }

    fn matches_type(&self, ty: &TypeKind) -> bool {
        match self {
            Self::Project(nominal) => ty == &nominal.ty(),
            Self::Environment { record } => ty
                .semantic_identity_digest()
                .is_ok_and(|digest| digest == record.semantic_type()),
            Self::VariantPayload { payload, .. } => matches!(ty,
                TypeKind::VariantPayload(actual) if actual.as_ref() == payload),
        }
    }

    pub(crate) const fn project_nominal(&self) -> Option<&CheckedProjectNominal> {
        match self {
            Self::Project(nominal) => Some(nominal),
            Self::Environment { .. } | Self::VariantPayload { .. } => None,
        }
    }

    pub(crate) const fn field_count(&self) -> Option<u32> {
        match self {
            Self::Project(_) => None,
            Self::Environment { record } => Some(record.field_count()),
            Self::VariantPayload { field_count, .. } => Some(*field_count),
        }
    }

    pub(crate) fn variant_payload(payload: crate::types::VariantPayloadType) -> Option<Self> {
        let fields = payload.shape().record_fields()?;
        let field_count = u32::try_from(fields.len()).ok()?;
        Some(Self::VariantPayload {
            payload,
            field_count,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedRecordPattern {
    ty: TypeKind,
    owner: PreparedRecordPatternOwner,
    fields: Box<[PreparedRecordPatternField]>,
    rest: PreparedRecordPatternRest,
}

impl PreparedRecordPattern {
    pub(crate) fn try_new(
        ty: TypeKind,
        owner: PreparedRecordPatternOwner,
        fields: Box<[PreparedRecordPatternField]>,
        rest: PreparedRecordPatternRest,
    ) -> Option<Self> {
        if !owner.matches_type(&ty)
            || owner.field_count().is_some_and(|field_count| {
                usize::try_from(field_count).ok().is_none_or(|field_count| {
                    fields.len() > field_count
                        || (matches!(rest, PreparedRecordPatternRest::Absent)
                            && fields.len() != field_count)
                })
            })
        {
            return None;
        }
        Some(Self {
            ty,
            owner,
            fields,
            rest,
        })
    }
    pub(crate) const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub(crate) const fn owner(&self) -> &PreparedRecordPatternOwner {
        &self.owner
    }
    pub(crate) const fn fields(&self) -> &[PreparedRecordPatternField] {
        &self.fields
    }
    pub(crate) const fn rest(&self) -> PreparedRecordPatternRest {
        self.rest
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        TypeKind,
        PreparedRecordPatternOwner,
        Box<[PreparedRecordPatternField]>,
        PreparedRecordPatternRest,
    ) {
        (self.ty, self.owner, self.fields, self.rest)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedPatternFact {
    Complete(CheckedPattern),
    Variant(PreparedVariantPattern),
    Record(PreparedRecordPattern),
}

impl From<CheckedPattern> for PreparedPatternFact {
    fn from(value: CheckedPattern) -> Self {
        Self::Complete(value)
    }
}

impl PreparedPatternFact {
    pub(crate) const fn ty(&self) -> &TypeKind {
        match self {
            Self::Complete(value) => value.ty(),
            Self::Variant(value) => value.ty(),
            Self::Record(value) => value.ty(),
        }
    }

    pub(crate) fn into_complete(self) -> Result<CheckedPattern, Self> {
        match self {
            Self::Complete(value) => Ok(value),
            other => Err(other),
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Complete(value) => value.visit_types(visitor),
            Self::Variant(value) => {
                visitor(value.ty())?;
                value.owner().visit_types(visitor)
            }
            Self::Record(value) => {
                visitor(value.ty())?;
                if let Some(nominal) = value.owner().project_nominal() {
                    nominal.visit_types(visitor)?;
                }
                for field in value.fields() {
                    visitor(field.field_type())?;
                }
                Ok(())
            }
        }
    }
}
