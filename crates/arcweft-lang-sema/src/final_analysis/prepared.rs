//! Private, consumable semantic facts that require the project-wide C2 seal.

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    identity::{ExprId, ItemId, LocalId, PatternId},
    leaf::HirName,
};

use crate::{
    effects::EffectSet,
    env::nominal::AcceptedEnvironmentRecordIdentity,
    record_field::CheckedRecordFieldSemanticId,
    types::{SemanticTypeDigest, TypeKind},
};

use super::{
    CheckedExpression, CheckedExpressionResolution, CheckedPattern, CheckedProjectNominal,
    CheckedStatement, CheckedTypeSelection,
};

#[path = "prepared/evaluated_effect.rs"]
mod evaluated_effect;
pub(crate) use evaluated_effect::PreparedEvaluatedEffect;
#[path = "prepared/dialogue.rs"]
mod dialogue;
pub(crate) use dialogue::{
    PreparedDialogueApplication, PreparedDialogueEffectSite, PreparedDialogueLinePlan,
};

/// Common checked expression state retained while a projection-dependent row
/// is awaiting the one project-wide seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedExpressionShell {
    ty: TypeKind,
    type_selection: CheckedTypeSelection,
    effects: EffectSet,
}

impl PreparedExpressionShell {
    pub(crate) const fn new(
        ty: TypeKind,
        type_selection: CheckedTypeSelection,
        effects: EffectSet,
    ) -> Self {
        Self {
            ty,
            type_selection,
            effects,
        }
    }

    pub(crate) const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub(crate) const fn type_selection(&self) -> CheckedTypeSelection {
        self.type_selection
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub(crate) fn into_parts(self) -> (TypeKind, CheckedTypeSelection, EffectSet) {
        (self.ty, self.type_selection, self.effects)
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
    pub(crate) fn with_type(self, ty: TypeKind) -> Self {
        let (_, type_selection, effects) = self.shell.into_parts();
        Self {
            shell: PreparedExpressionShell::new(ty, type_selection, effects),
            diagnostic_name: self.diagnostic_name,
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
        let value_type = ty.semantic_identity_digest();
        Self {
            reference,
            shell: PreparedExpressionShell::new(ty, type_selection, EffectSet::new()),
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

/// One declaration-ordered case carried without runtime layout identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantCaseSeed {
    ordinal: u32,
    payload: Option<TypeKind>,
    diagnostic_name: Option<String>,
}

impl PreparedVariantCaseSeed {
    pub(crate) const fn new(
        ordinal: u32,
        payload: Option<TypeKind>,
        diagnostic_name: Option<String>,
    ) -> Self {
        Self {
            ordinal,
            payload,
            diagnostic_name,
        }
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn payload(&self) -> Option<&TypeKind> {
        self.payload.as_ref()
    }

    pub(crate) fn diagnostic_name(&self) -> Option<&str> {
        self.diagnostic_name.as_deref()
    }
}

/// Complete project-enum owner evidence before layout-backed case IDs exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectVariantOwnerSeed {
    nominal: CheckedProjectNominal,
    cases: Box<[PreparedVariantCaseSeed]>,
}

impl PreparedProjectVariantOwnerSeed {
    pub(crate) fn try_new(
        nominal: CheckedProjectNominal,
        cases: impl Into<Box<[PreparedVariantCaseSeed]>>,
    ) -> Option<Self> {
        let cases = cases.into();
        if !cases.iter().enumerate().all(|(ordinal, case)| {
            u32::try_from(ordinal).is_ok_and(|ordinal| ordinal == case.ordinal)
                && case
                    .payload
                    .as_ref()
                    .is_none_or(|payload| !payload.contains_nominal_poison())
        }) {
            return None;
        }
        Some(Self { nominal, cases })
    }

    pub(crate) const fn nominal(&self) -> &CheckedProjectNominal {
        &self.nominal
    }

    pub(crate) const fn cases(&self) -> &[PreparedVariantCaseSeed] {
        &self.cases
    }

    pub(crate) fn into_parts(self) -> (CheckedProjectNominal, Box<[PreparedVariantCaseSeed]>) {
        (self.nominal, self.cases)
    }

    fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.nominal.visit_types(visitor)?;
        for case in self.cases() {
            if let Some(payload) = case.payload() {
                visitor(payload)?;
            }
        }
        Ok(())
    }
}

/// One project-enum expression awaiting digest-ordered owner sealing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectVariantExpression {
    shell: PreparedExpressionShell,
    owner: PreparedProjectVariantOwnerSeed,
    selected_ordinal: u32,
}

impl PreparedProjectVariantExpression {
    pub(crate) fn try_new(
        shell: PreparedExpressionShell,
        owner: PreparedProjectVariantOwnerSeed,
        selected_ordinal: u32,
    ) -> Option<Self> {
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

    pub(crate) const fn owner(&self) -> &PreparedProjectVariantOwnerSeed {
        &self.owner
    }

    pub(crate) const fn selected_ordinal(&self) -> u32 {
        self.selected_ordinal
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedExpressionShell,
        PreparedProjectVariantOwnerSeed,
        u32,
    ) {
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

/// Analyzer-owned expression fact. Only `Complete` may enter the published
/// report; every other row is consumed by the private project seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedExpressionFact {
    Complete(CheckedExpression),
    DialogueApplication(PreparedDialogueApplication),
    Method(PreparedMethodExpression),
    Entry(PreparedEntryExpression),
    ProjectVariant(PreparedProjectVariantExpression),
    ProjectField(PreparedProjectFieldExpression),
    ProjectRecord(PreparedProjectRecordExpression),
}

impl From<CheckedExpression> for PreparedExpressionFact {
    fn from(value: CheckedExpression) -> Self {
        Self::Complete(value)
    }
}

impl PreparedExpressionFact {
    pub(crate) const fn ty(&self) -> &TypeKind {
        match self {
            Self::Complete(value) => value.ty(),
            Self::DialogueApplication(value) => value.shell().ty(),
            Self::Method(value) => value.shell().ty(),
            Self::Entry(value) => value.shell().ty(),
            Self::ProjectVariant(value) => value.shell().ty(),
            Self::ProjectField(value) => value.shell().ty(),
            Self::ProjectRecord(value) => value.shell().ty(),
        }
    }

    pub(crate) const fn type_selection(&self) -> CheckedTypeSelection {
        match self {
            Self::Complete(value) => value.type_selection(),
            Self::DialogueApplication(value) => value.shell().type_selection(),
            Self::Method(value) => value.shell().type_selection(),
            Self::Entry(value) => value.shell().type_selection(),
            Self::ProjectVariant(value) => value.shell().type_selection(),
            Self::ProjectField(value) => value.shell().type_selection(),
            Self::ProjectRecord(value) => value.shell().type_selection(),
        }
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        match self {
            Self::Complete(value) => value.effects(),
            Self::DialogueApplication(value) => value.shell().effects(),
            Self::Method(value) => value.shell().effects(),
            Self::Entry(value) => value.shell().effects(),
            Self::ProjectVariant(value) => value.shell().effects(),
            Self::ProjectField(value) => value.shell().effects(),
            Self::ProjectRecord(value) => value.shell().effects(),
        }
    }

    pub(crate) const fn complete(&self) -> Option<&CheckedExpression> {
        match self {
            Self::Complete(value) => Some(value),
            Self::Method(_)
            | Self::DialogueApplication(_)
            | Self::Entry(_)
            | Self::ProjectVariant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_) => None,
        }
    }

    pub(crate) fn into_complete(self) -> Result<CheckedExpression, Self> {
        match self {
            Self::Complete(value) => Ok(value),
            other => Err(other),
        }
    }

    pub(crate) const fn checked_resolution(&self) -> Option<&CheckedExpressionResolution> {
        match self {
            Self::Complete(value) => Some(value.resolution()),
            Self::DialogueApplication(_)
            | Self::Method(_)
            | Self::Entry(_)
            | Self::ProjectVariant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_) => None,
        }
    }

    pub(crate) fn reusable_for_parametric_expectation(&self, expected: &TypeKind) -> bool {
        self.type_selection() != CheckedTypeSelection::Expected
            || self.ty().semantic_identity_digest() == expected.semantic_identity_digest()
    }

    pub(crate) const fn checked_call_site(
        &self,
        owner: ExprId,
    ) -> Option<crate::callable::CheckedCallSite> {
        match self.checked_resolution() {
            Some(resolution) => resolution.checked_call_site(owner),
            None => match self {
                Self::DialogueApplication(_) => {
                    Some(crate::callable::CheckedCallSite::DialogueApplication(owner))
                }
                _ => None,
            },
        }
    }

    pub(crate) const fn selected_postfix_candidate(&self) -> Option<ExprId> {
        match self {
            Self::Complete(value) => value.selected_postfix_candidate(),
            Self::DialogueApplication(_)
            | Self::Method(_)
            | Self::Entry(_)
            | Self::ProjectVariant(_)
            | Self::ProjectField(_)
            | Self::ProjectRecord(_) => None,
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Complete(value) => value.visit_types(visitor),
            Self::DialogueApplication(value) => {
                value.target().visit_types(visitor)?;
                if let Some(patch) = value.application_patch() {
                    patch.visit_types(visitor)?;
                }
                visitor(value.line_result())
            }
            Self::Method(value) => visitor(value.shell().ty()),
            Self::Entry(value) => visitor(value.shell().ty()),
            Self::ProjectVariant(value) => {
                visitor(value.shell().ty())?;
                value.owner().visit_types(visitor)
            }
            Self::ProjectField(value) => {
                visitor(value.shell().ty())?;
                value.nominal().visit_types(visitor)?;
                visitor(value.field_type())
            }
            Self::ProjectRecord(value) => {
                visitor(value.shell().ty())?;
                value.nominal().visit_types(visitor)?;
                for field in value.fields() {
                    visitor(field.field_type())?;
                }
                Ok(())
            }
        }
    }
}

/// One direct-local project-field assignment awaiting the project-wide field
/// coordinate seal.
///
/// The analyzer has already admitted the direct local, its exact checked
/// project nominal, and equal target/value types. The runtime field coordinate
/// remains deliberately absent until the same seal that finalizes the target
/// [`PreparedExpressionFact::ProjectField`] row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedAssignmentStatement {
    effects: EffectSet,
    local: LocalId,
    nominal: CheckedProjectNominal,
    target: ExprId,
    value: ExprId,
    field_type: TypeKind,
}

impl PreparedAssignmentStatement {
    pub(crate) const fn new(
        effects: EffectSet,
        local: LocalId,
        nominal: CheckedProjectNominal,
        target: ExprId,
        value: ExprId,
        field_type: TypeKind,
    ) -> Self {
        Self {
            effects,
            local,
            nominal,
            target,
            value,
            field_type,
        }
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        EffectSet,
        LocalId,
        CheckedProjectNominal,
        ExprId,
        ExprId,
        TypeKind,
    ) {
        (
            self.effects,
            self.local,
            self.nominal,
            self.target,
            self.value,
            self.field_type,
        )
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.nominal.visit_types(visitor)?;
        visitor(&self.field_type)
    }
}

/// Analyzer-owned statement fact. Only `Complete` may enter the published
/// report; an assignment is consumed after its target field receives the
/// issuer-backed runtime coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedStatementFact {
    Complete(CheckedStatement),
    Assignment(PreparedAssignmentStatement),
    EvaluatedEffect(PreparedEvaluatedEffect),
}

impl From<CheckedStatement> for PreparedStatementFact {
    fn from(value: CheckedStatement) -> Self {
        Self::Complete(value)
    }
}

impl PreparedStatementFact {
    pub(crate) fn extend_effects(&self, effects: &mut EffectSet) {
        match self {
            Self::Complete(value) => {
                effects.union_with(value.effects());
            }
            Self::Assignment(value) => {
                effects.union_with(value.effects());
            }
            Self::EvaluatedEffect(_) => {}
        }
    }

    pub(crate) fn into_complete(self) -> Result<CheckedStatement, Self> {
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
            Self::Assignment(value) => value.visit_types(visitor),
            Self::EvaluatedEffect(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedProjectVariantPattern {
    ty: TypeKind,
    owner: PreparedProjectVariantOwnerSeed,
    selected_ordinal: u32,
}

impl PreparedProjectVariantPattern {
    pub(crate) fn try_new(
        ty: TypeKind,
        owner: PreparedProjectVariantOwnerSeed,
        selected_ordinal: u32,
    ) -> Option<Self> {
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
    pub(crate) const fn owner(&self) -> &PreparedProjectVariantOwnerSeed {
        &self.owner
    }
    pub(crate) fn into_parts(self) -> (TypeKind, PreparedProjectVariantOwnerSeed, u32) {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedRecordPatternFieldIdentity {
    Project {
        declaration_ordinal: u32,
    },
    Environment {
        declaration_ordinal: u32,
        semantic_id: CheckedRecordFieldSemanticId,
    },
    VariantPayload {
        declaration_ordinal: u32,
        semantic_id: CheckedRecordFieldSemanticId,
    },
}

impl PreparedRecordPatternFieldIdentity {
    pub(crate) const fn declaration_ordinal(self) -> u32 {
        match self {
            Self::Project {
                declaration_ordinal,
            }
            | Self::Environment {
                declaration_ordinal,
                ..
            }
            | Self::VariantPayload {
                declaration_ordinal,
                ..
            } => declaration_ordinal,
        }
    }

    pub(crate) const fn semantic_id(self) -> Option<CheckedRecordFieldSemanticId> {
        match self {
            Self::Project { .. } => None,
            Self::Environment { semantic_id, .. } | Self::VariantPayload { semantic_id, .. } => {
                Some(semantic_id)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedRecordPatternField {
    source_ordinal: u32,
    identity: PreparedRecordPatternFieldIdentity,
    field_type: TypeKind,
    source: PreparedRecordPatternSource,
}

impl PreparedRecordPatternField {
    pub(crate) const fn project(
        source_ordinal: u32,
        declaration_ordinal: u32,
        field_type: TypeKind,
        source: PreparedRecordPatternSource,
    ) -> Self {
        Self {
            source_ordinal,
            identity: PreparedRecordPatternFieldIdentity::Project {
                declaration_ordinal,
            },
            field_type,
            source,
        }
    }
    pub(crate) const fn environment(
        source_ordinal: u32,
        declaration_ordinal: u32,
        semantic_id: CheckedRecordFieldSemanticId,
        field_type: TypeKind,
        source: PreparedRecordPatternSource,
    ) -> Self {
        Self {
            source_ordinal,
            identity: PreparedRecordPatternFieldIdentity::Environment {
                declaration_ordinal,
                semantic_id,
            },
            field_type,
            source,
        }
    }
    pub(crate) const fn variant_payload(
        source_ordinal: u32,
        declaration_ordinal: u32,
        semantic_id: CheckedRecordFieldSemanticId,
        field_type: TypeKind,
        source: PreparedRecordPatternSource,
    ) -> Self {
        Self {
            source_ordinal,
            identity: PreparedRecordPatternFieldIdentity::VariantPayload {
                declaration_ordinal,
                semantic_id,
            },
            field_type,
            source,
        }
    }
    pub(crate) const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }
    pub(crate) const fn identity(&self) -> PreparedRecordPatternFieldIdentity {
        self.identity
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
        PreparedRecordPatternFieldIdentity,
        TypeKind,
        PreparedRecordPatternSource,
    ) {
        (
            self.source_ordinal,
            self.identity,
            self.field_type,
            self.source,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedRecordPatternOwner {
    Project(CheckedProjectNominal),
    Environment {
        record: AcceptedEnvironmentRecordIdentity,
    },
    VariantPayload {
        payload: crate::types::VariantPayloadType,
        semantic_type: SemanticTypeDigest,
        field_count: u32,
    },
}

impl PreparedRecordPatternOwner {
    pub(crate) const fn semantic_type(&self) -> SemanticTypeDigest {
        match self {
            Self::Project(nominal) => nominal.identity(),
            Self::Environment { record } => record.semantic_type(),
            Self::VariantPayload { semantic_type, .. } => *semantic_type,
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
        let semantic_type =
            TypeKind::VariantPayload(Box::new(payload.clone())).semantic_identity_digest();
        Some(Self::VariantPayload {
            payload,
            semantic_type,
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
        if ty.semantic_identity_digest() != owner.semantic_type()
            || fields.iter().any(|field| {
                !matches!(
                    (&owner, field.identity()),
                    (
                        PreparedRecordPatternOwner::Project(_),
                        PreparedRecordPatternFieldIdentity::Project { .. }
                    ) | (
                        PreparedRecordPatternOwner::Environment { .. },
                        PreparedRecordPatternFieldIdentity::Environment { .. }
                    ) | (
                        PreparedRecordPatternOwner::VariantPayload { .. },
                        PreparedRecordPatternFieldIdentity::VariantPayload { .. }
                    )
                )
            })
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
    ProjectVariant(PreparedProjectVariantPattern),
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
            Self::ProjectVariant(value) => value.ty(),
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
            Self::ProjectVariant(value) => {
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
