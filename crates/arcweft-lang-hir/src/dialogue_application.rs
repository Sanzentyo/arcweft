//! Final dialogue-content and `RichText` records for the shared HIR expression arena.
//!
//! Source sites are owned by the HIR source index. Arena liveness, expression
//! kind checks, lexical-scope admission, and `RichText` limit accounting are
//! reported to the lowering transaction through [`HirDialogueTransactionContext`].

use std::collections::BTreeSet;

use crate::expr::{
    HirCallArgument, HirCallArgumentOrdinal, HirCallArgumentOrdinalError, HirCallInvocation,
    HirCallTypeArgument, HirExprKind, HirExpressionTypeRoot,
};
use crate::identity::{ExprId, HirModuleId, ScopeId, StmtId, SyntheticRole, TypeId};
use crate::leaf::HirName;
use crate::module::HirModule;

mod content;
mod rich_text;
pub(crate) mod ruby;

pub use self::content::{
    HirDialogueContent, HirDialogueContentError, HirDialogueContentId, HirDialogueIssue,
    HirDialogueMark, HirDialogueMarkId, HirDialogueMarkName, HirDialogueMarkOrdinal,
    HirDialogueNode, HirDialogueNodeId, HirDialogueNodeKind, HirDialoguePointAction,
    HirDialoguePointActionArgument, HirDialoguePointActionArgumentId,
    HirDialoguePointActionIdentity, HirDialoguePointActionPayload, HirLineBreakKind,
    HirRawLiteralBody, HirTextFragment,
};
pub use self::rich_text::{
    HirDialogueControl, HirRichTextArgumentIssue, HirRichTextHostEvent, HirRichTextIssue,
    HirRichTextValue,
};

/// Exact semantic operand that identifies an Object call's nominal type.
///
/// The source expression remains an HIR child for evaluation and diagnostics;
/// the separate type root is semantic-only and must never enter runtime
/// ownership. Keeping both IDs here prevents consumers from rediscovering the
/// discriminator by argument name or source spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirContentCallNominalDiscriminator {
    argument: HirCallArgumentOrdinal,
    source: ExprId,
    semantic_only: TypeId,
}

/// Recovery-aware required Object discriminator. Recognized Object calls
/// never fall back to a generic callable merely because their `type` operand
/// is malformed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRequiredContentCallNominalDiscriminator {
    Present(HirContentCallNominalDiscriminator),
    Invalid,
}

impl HirRequiredContentCallNominalDiscriminator {
    pub const fn present(self) -> Option<HirContentCallNominalDiscriminator> {
        match self {
            Self::Present(value) => Some(value),
            Self::Invalid => None,
        }
    }
}

/// Semantic evidence retained by HIR for an attached content call.
///
/// Content callable identity belongs to the presentation catalog and the
/// semantic resolver. HIR retains only the one source-shape fact that cannot
/// be recovered from a generic call without reinterpreting its arguments:
/// the exact nominal discriminator of the canonical `object` root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirContentCallSemanticEvidence {
    None,
    TextProxyObject {
        nominal_discriminator: HirRequiredContentCallNominalDiscriminator,
    },
}

impl HirContentCallSemanticEvidence {
    pub const fn nominal_discriminator(self) -> Option<HirContentCallNominalDiscriminator> {
        match self {
            Self::None => None,
            Self::TextProxyObject {
                nominal_discriminator,
            } => nominal_discriminator.present(),
        }
    }

    pub const fn has_invalid_required_payload(self) -> bool {
        matches!(
            self,
            Self::TextProxyObject {
                nominal_discriminator: HirRequiredContentCallNominalDiscriminator::Invalid
            }
        )
    }
}

impl HirContentCallNominalDiscriminator {
    pub(crate) const fn new(
        argument: HirCallArgumentOrdinal,
        source: ExprId,
        semantic_only: TypeId,
    ) -> Self {
        Self {
            argument,
            source,
            semantic_only,
        }
    }

    /// Authored argument ordinal carrying the nominal discriminator.
    pub const fn argument(self) -> HirCallArgumentOrdinal {
        self.argument
    }

    /// Expression ID of the authored nominal type operand.
    pub const fn source(self) -> ExprId {
        self.source
    }

    /// Semantic-only HIR type root for the nominal discriminator.
    pub const fn semantic_only(self) -> TypeId {
        self.semantic_only
    }

    /// Alias emphasizing that the field is a type-arena root.
    pub const fn type_root(self) -> TypeId {
        self.semantic_only
    }
}

/// The semantic family of one attached content application.
///
/// Dialogue-line metadata is deliberately nested in its family variant. A
/// content call therefore has no representational slot for a line plan or
/// dialogue coordinates, rather than carrying an invalid empty/optional
/// projection of either one.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAttachedContentApplicationFamily {
    DialogueLine {
        target: ExprId,
        plan: Option<HirLinePlan>,
        coordinates: Box<[HirDialogueCoordinate]>,
    },
    ContentCall {
        invocation: HirCallInvocation,
        evidence: HirContentCallSemanticEvidence,
    },
}

/// Whether the attached body delimiter was present in source. Empty present
/// content and an omitted body both own an empty HIR content value, so this
/// typed bit is retained alongside the application instead of being inferred
/// from that value later.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAttachedContentBodyPresence {
    Absent,
    Present,
}

impl HirAttachedContentApplicationFamily {
    pub const fn is_dialogue_line(&self) -> bool {
        matches!(self, Self::DialogueLine { .. })
    }

    pub const fn is_content_call(&self) -> bool {
        matches!(self, Self::ContentCall { .. })
    }

    pub const fn invocation(&self) -> Option<&HirCallInvocation> {
        match self {
            Self::DialogueLine { .. } => None,
            Self::ContentCall { invocation, .. } => Some(invocation),
        }
    }

    pub const fn content_call_evidence(&self) -> Option<HirContentCallSemanticEvidence> {
        match self {
            Self::DialogueLine { .. } => None,
            Self::ContentCall { evidence, .. } => Some(*evidence),
        }
    }
}

/// One attached content application in the shared expression arena.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirAttachedContentApplication {
    content: HirDialogueContent,
    family: HirAttachedContentApplicationFamily,
    body_presence: HirAttachedContentBodyPresence,
}

impl HirAttachedContentApplication {
    pub(crate) fn try_new_with_body_presence(
        owner: ExprId,
        content: HirDialogueContent,
        family: HirAttachedContentApplicationFamily,
        body_presence: HirAttachedContentBodyPresence,
    ) -> Result<Self, HirDialogueInvariantError> {
        if content.id().owner() != owner {
            return Err(HirDialogueInvariantError::InvalidContentOwner);
        }
        match &family {
            HirAttachedContentApplicationFamily::DialogueLine { coordinates, .. } => {
                validate_coordinate_order(coordinates)?;
            }
            HirAttachedContentApplicationFamily::ContentCall {
                invocation,
                evidence,
            } if invocation.callee().value_expression().is_none() => {
                return Err(HirDialogueInvariantError::InvalidContentCallInvocation);
            }
            HirAttachedContentApplicationFamily::ContentCall { .. } => {}
        }
        if let HirAttachedContentApplicationFamily::ContentCall {
            invocation,
            evidence,
        } = &family
            && let Some(nominal_discriminator) = evidence.nominal_discriminator()
        {
            let argument = invocation
                .arguments()
                .get(usize::from(nominal_discriminator.argument().get()))
                .ok_or(HirDialogueInvariantError::InvalidContentCallSemanticEvidence)?;
            if argument.value() != nominal_discriminator.source() {
                return Err(HirDialogueInvariantError::InvalidContentCallSemanticEvidence);
            }
            if nominal_discriminator.source().module() != owner.module()
                || nominal_discriminator.semantic_only().module() != owner.module()
            {
                return Err(HirDialogueInvariantError::InvalidContentCallSemanticEvidence);
            }
        }
        let application = Self {
            content,
            family,
            body_presence,
        };
        application
            .validate_module(owner.module())
            .map_err(|actual| HirDialogueInvariantError::ForeignChild {
                expected: owner.module(),
                actual,
            })?;
        Ok(application)
    }

    /// Returns the complete ordered dialogue content.
    pub const fn content(&self) -> &HirDialogueContent {
        &self.content
    }

    /// Returns the explicit attached-content semantic family.
    pub const fn family(&self) -> &HirAttachedContentApplicationFamily {
        &self.family
    }

    pub const fn body_presence(&self) -> HirAttachedContentBodyPresence {
        self.body_presence
    }

    /// Returns whether this application produces a dialogue line.
    pub const fn is_dialogue_line(&self) -> bool {
        self.family.is_dialogue_line()
    }

    /// Returns whether this application is an attached content call.
    pub const fn is_content_call(&self) -> bool {
        self.family.is_content_call()
    }

    /// Returns all runtime-bearing typed roots authored by this application.
    /// Point-action arguments are value-only; content-call type arguments
    /// remain owned by the invocation itself.
    pub(crate) fn direct_type_roots(&self) -> Vec<HirExpressionTypeRoot> {
        let mut roots = Vec::new();
        if let HirAttachedContentApplicationFamily::ContentCall {
            invocation,
            evidence,
        } = &self.family
        {
            roots.extend(
                invocation
                    .callee()
                    .associated_parts()
                    .and_then(|(receiver, _, _)| receiver.type_id())
                    .into_iter()
                    .chain(
                        invocation
                            .explicit_type_application()
                            .arguments()
                            .iter()
                            .filter_map(HirCallTypeArgument::type_id),
                    )
                    .map(HirExpressionTypeRoot::runtime),
            );
            if let Some(discriminator) = evidence.nominal_discriminator() {
                roots.push(HirExpressionTypeRoot::semantic_only(
                    discriminator.semantic_only(),
                ));
            }
        }
        roots
    }

    pub(crate) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        match &self.family {
            HirAttachedContentApplicationFamily::DialogueLine {
                target,
                plan,
                coordinates,
            } => {
                validate_module(expected, target.module())?;
                if let Some(plan) = plan {
                    plan.validate_module(expected)?;
                }
                for coordinate in coordinates {
                    validate_module(expected, coordinate.value.module())?;
                }
            }
            HirAttachedContentApplicationFamily::ContentCall {
                invocation,
                evidence,
            } => {
                invocation.validate_module(expected).map_err(|_| expected)?;
                if let Some(nominal_discriminator) = evidence.nominal_discriminator() {
                    validate_module(expected, nominal_discriminator.source().module())?;
                    validate_module(expected, nominal_discriminator.semantic_only().module())?;
                }
            }
        }
        self.content.validate_module(expected)?;
        Ok(())
    }

    pub(crate) fn has_recovery(&self) -> bool {
        (matches!(
            self.family,
            HirAttachedContentApplicationFamily::DialogueLine { .. }
        ) && matches!(self.body_presence, HirAttachedContentBodyPresence::Absent))
            || self.content.has_recovery()
            || match &self.family {
                HirAttachedContentApplicationFamily::DialogueLine { plan, .. } => {
                    plan.as_ref().is_some_and(HirLinePlan::has_recovery)
                }
                HirAttachedContentApplicationFamily::ContentCall { invocation, .. } => {
                    invocation.contains_recovery_payload()
                        || self.family.content_call_evidence().is_some_and(
                            HirContentCallSemanticEvidence::has_invalid_required_payload,
                        )
                }
            }
    }

    pub(crate) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        self.content.validate_transaction(context)?;
        match &self.family {
            HirAttachedContentApplicationFamily::DialogueLine {
                target,
                plan,
                coordinates,
            } => {
                context
                    .require(HirDialogueTransactionRequirement::Expression {
                        id: *target,
                        expected: HirDialogueExpressionExpectation::Unrestricted,
                    })
                    .map_err(HirDialogueTransactionError::Context)?;
                for coordinate in coordinates {
                    context
                        .require(HirDialogueTransactionRequirement::Expression {
                            id: coordinate.value,
                            expected: HirDialogueExpressionExpectation::Unrestricted,
                        })
                        .map_err(HirDialogueTransactionError::Context)?;
                }
                if let Some(plan) = plan {
                    plan.validate_transaction(context)?;
                }
            }
            HirAttachedContentApplicationFamily::ContentCall {
                invocation,
                evidence,
            } => {
                if let Some(target) = invocation.callee().value_expression() {
                    context
                        .require(HirDialogueTransactionRequirement::Expression {
                            id: target,
                            expected: HirDialogueExpressionExpectation::Unrestricted,
                        })
                        .map_err(HirDialogueTransactionError::Context)?;
                }
                if let Some(nominal_discriminator) = evidence.nominal_discriminator() {
                    context
                        .require(HirDialogueTransactionRequirement::Type(
                            nominal_discriminator.semantic_only(),
                        ))
                        .map_err(HirDialogueTransactionError::Context)?;
                }
                for argument in invocation.arguments() {
                    context
                        .require(HirDialogueTransactionRequirement::Expression {
                            id: argument.value(),
                            expected: HirDialogueExpressionExpectation::Unrestricted,
                        })
                        .map_err(HirDialogueTransactionError::Context)?;
                }
            }
        }
        Ok(())
    }
}

/// One immediate outer-call configuration coordinate.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueCoordinate {
    kind: HirDialogueCoordinateKind,
    argument: HirCallArgumentOrdinal,
    value: ExprId,
}

impl HirDialogueCoordinate {
    pub(crate) fn from_immediate_arguments(
        arguments: &[HirCallArgument],
    ) -> Result<Box<[Self]>, HirCallArgumentOrdinalError> {
        arguments
            .iter()
            .enumerate()
            .filter_map(|(ordinal, argument)| {
                let kind = match argument.resolved_name().map(HirName::as_str) {
                    Some("id") => HirDialogueCoordinateKind::Id,
                    Some("text_key") => HirDialogueCoordinateKind::TextKey,
                    _ => return None,
                };
                Some(
                    HirCallArgumentOrdinal::try_new(ordinal).map(|argument_ordinal| Self {
                        kind,
                        argument: argument_ordinal,
                        value: argument.value(),
                    }),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    /// Returns the reserved coordinate family.
    pub const fn kind(&self) -> HirDialogueCoordinateKind {
        self.kind
    }

    /// Returns the authored ordinary-call argument position.
    pub const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    /// Returns the unchanged same-arena value expression.
    pub const fn value(&self) -> ExprId {
        self.value
    }
}

/// Reserved dialogue configuration coordinate families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueCoordinateKind {
    Id,
    TextKey,
}

/// Typed pre-sema evidence for one immediate dialogue coordinate. The HIR
/// query reports the existing value shape only; semantic identity and project
/// acceptance remain owned by later seals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDialogueCoordinateValueRef {
    IdRef(crate::leaf::HirIdRef),
    Runtime(ExprId),
    Error(ExprId),
}

impl HirModule {
    /// Classifies one immediate coordinate without reparsing source text or
    /// fabricating a line identity from a runtime expression.
    pub fn dialogue_coordinate_value(
        &self,
        coordinate: &HirDialogueCoordinate,
    ) -> Result<HirDialogueCoordinateValueRef, crate::identity::IdResolveError> {
        let expression = self.resolve_expr(coordinate.value())?;
        match expression.kind() {
            HirExprKind::EntityReference(crate::leaf::HirIdRefValue::Resolved(reference))
                if !expression.is_poisoned() =>
            {
                Ok(HirDialogueCoordinateValueRef::IdRef(reference.clone()))
            }
            HirExprKind::EntityReference(crate::leaf::HirIdRefValue::Recovered(_))
            | HirExprKind::Error(_) => Ok(HirDialogueCoordinateValueRef::Error(coordinate.value())),
            _ if expression.is_poisoned() => {
                Ok(HirDialogueCoordinateValueRef::Error(coordinate.value()))
            }
            _ => Ok(HirDialogueCoordinateValueRef::Runtime(coordinate.value())),
        }
    }
}

/// HIR-owner-issued projection of the immediate Dialogue metadata arguments
/// onto the exact inner target Call. Downstream layers consume this carrier
/// instead of rediscovering `id`/`text_key` from names or guessing edge
/// ownership from expression membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueApplicationMetadataProjection {
    application: ExprId,
    target_call: ExprId,
    coordinates: Box<[HirDialogueCoordinate]>,
}

impl HirDialogueApplicationMetadataProjection {
    pub const fn application(&self) -> ExprId {
        self.application
    }

    pub const fn target_call(&self) -> ExprId {
        self.target_call
    }

    pub const fn coordinates(&self) -> &[HirDialogueCoordinate] {
        &self.coordinates
    }
}

/// Failure to issue an exact application-to-target metadata projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueApplicationMetadataProjectionError {
    UnknownApplication,
    NotDialogueApplication,
    UnknownTarget,
    TargetNotCall,
    ArgumentOrdinalOverflow,
    ArgumentOrdinalMismatch,
    ArgumentIdentityMismatch,
    CoordinateKindMismatch,
    DuplicateCoordinate,
    CoordinateInventoryMismatch,
}

impl HirModule {
    /// Issues the exact metadata edge projection for one Dialogue content
    /// application in this module and generation.
    pub fn dialogue_application_metadata_projection(
        &self,
        owner: ExprId,
    ) -> Result<
        HirDialogueApplicationMetadataProjection,
        HirDialogueApplicationMetadataProjectionError,
    > {
        let expression = self
            .resolve_expr(owner)
            .map_err(|_| HirDialogueApplicationMetadataProjectionError::UnknownApplication)?;
        let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
            return Err(HirDialogueApplicationMetadataProjectionError::NotDialogueApplication);
        };
        if !application.is_dialogue_line() {
            return Err(HirDialogueApplicationMetadataProjectionError::NotDialogueApplication);
        }
        let HirAttachedContentApplicationFamily::DialogueLine { target, .. } = application.family()
        else {
            return Err(HirDialogueApplicationMetadataProjectionError::NotDialogueApplication);
        };
        let target_id = *target;
        let target = self
            .resolve_expr(target_id)
            .map_err(|_| HirDialogueApplicationMetadataProjectionError::UnknownTarget)?;
        let HirExprKind::Call(call) = target.kind() else {
            return Err(HirDialogueApplicationMetadataProjectionError::TargetNotCall);
        };
        let coordinates = validate_application_metadata_projection(application, call)?;
        Ok(HirDialogueApplicationMetadataProjection {
            application: owner,
            target_call: target_id,
            coordinates,
        })
    }
}

fn validate_application_metadata_projection(
    application: &HirAttachedContentApplication,
    call: &crate::expr::HirCallInvocation,
) -> Result<Box<[HirDialogueCoordinate]>, HirDialogueApplicationMetadataProjectionError> {
    let HirAttachedContentApplicationFamily::DialogueLine { coordinates, .. } =
        application.family()
    else {
        return Err(HirDialogueApplicationMetadataProjectionError::NotDialogueApplication);
    };
    let canonical = HirDialogueCoordinate::from_immediate_arguments(call.arguments())
        .map_err(|_| HirDialogueApplicationMetadataProjectionError::ArgumentOrdinalOverflow)?;
    let mut arguments = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for coordinate in coordinates {
        if !arguments.insert(coordinate.argument())
            || !sources.insert(coordinate.value())
            || !kinds.insert(coordinate.kind())
        {
            return Err(HirDialogueApplicationMetadataProjectionError::DuplicateCoordinate);
        }
        let argument = call
            .arguments()
            .get(usize::from(coordinate.argument().get()))
            .ok_or(HirDialogueApplicationMetadataProjectionError::ArgumentOrdinalMismatch)?;
        if argument.value() != coordinate.value() {
            return Err(HirDialogueApplicationMetadataProjectionError::ArgumentIdentityMismatch);
        }
        let expected = canonical
            .iter()
            .find(|expected| expected.argument() == coordinate.argument())
            .ok_or(HirDialogueApplicationMetadataProjectionError::CoordinateKindMismatch)?;
        if expected.kind() != coordinate.kind() {
            return Err(HirDialogueApplicationMetadataProjectionError::CoordinateKindMismatch);
        }
    }
    if canonical.as_ref() != coordinates.as_ref() {
        return Err(HirDialogueApplicationMetadataProjectionError::CoordinateInventoryMismatch);
    }
    Ok(canonical)
}

/// A typed line plan whose children use the module's existing arenas.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirLinePlan {
    root_scope: ScopeId,
    label: Option<HirName>,
    items: Box<[HirLinePlanItem]>,
}

impl HirLinePlan {
    pub(crate) fn try_new(
        root_scope: ScopeId,
        label: Option<HirName>,
        items: Box<[HirLinePlanItem]>,
    ) -> Result<Self, HirDialogueInvariantError> {
        validate_line_plan_items(root_scope.module(), &items).map_err(|actual| {
            HirDialogueInvariantError::ForeignChild {
                expected: root_scope.module(),
                actual,
            }
        })?;
        Ok(Self {
            root_scope,
            label,
            items,
        })
    }

    /// Returns the plan's one child block scope.
    pub const fn root_scope(&self) -> ScopeId {
        self.root_scope
    }

    /// Returns the optional semantic label.
    pub const fn label(&self) -> Option<&HirName> {
        self.label.as_ref()
    }

    /// Returns source-ordered plan items.
    pub const fn items(&self) -> &[HirLinePlanItem] {
        &self.items
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.root_scope.module())?;
        validate_line_plan_items(expected, &self.items)
    }

    fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::Scope(self.root_scope))
            .map_err(HirDialogueTransactionError::Context)?;
        report_line_plan_items(&self.items, context)
    }

    pub(crate) fn has_recovery(&self) -> bool {
        line_plan_items_have_recovery(&self.items)
    }
}

/// Semantic line-plan item projection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLinePlanItem {
    Init(Box<[StmtId]>),
    Thread(StmtId),
    On(StmtId),
    Statement(StmtId),
    CancelRule(StmtId),
    StartGroup(Box<[HirLinePlanItem]>),
    TogetherGroup(Box<[HirLinePlanItem]>),
    Error(StmtId),
}

/// One generic postfix bracket with exactly two bounded interpretations.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPostfixBracket {
    target: ExprId,
    candidates: HirPostfixBracketCandidates,
}

impl HirPostfixBracket {
    pub(crate) fn try_new(
        target: ExprId,
        candidates: HirPostfixBracketCandidates,
    ) -> Result<Self, HirDialogueInvariantError> {
        if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = candidates {
            if index == dialogue || index == target || dialogue == target {
                return Err(HirDialogueInvariantError::InvalidPostfixCandidate);
            }
            validate_module(target.module(), index.module()).map_err(|actual| {
                HirDialogueInvariantError::ForeignChild {
                    expected: target.module(),
                    actual,
                }
            })?;
            validate_module(target.module(), dialogue.module()).map_err(|actual| {
                HirDialogueInvariantError::ForeignChild {
                    expected: target.module(),
                    actual,
                }
            })?;
        }
        Ok(Self { target, candidates })
    }

    /// Returns the shared target excluded from both candidate inventories.
    pub const fn target(&self) -> ExprId {
        self.target
    }

    /// Returns the exact ambiguous or invalid two-result carrier.
    pub const fn candidates(&self) -> &HirPostfixBracketCandidates {
        &self.candidates
    }

    pub(crate) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.target.module())?;
        if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = self.candidates {
            validate_module(expected, index.module())?;
            validate_module(expected, dialogue.module())?;
        }
        Ok(())
    }

    pub(crate) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        owner: ExprId,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        self.validate_transaction_with_roles(
            owner,
            SyntheticRole::PostfixIndexCandidateExpression,
            SyntheticRole::DialogueContentCandidateExpression,
            context,
        )
    }

    pub(crate) fn validate_candidate_transaction<C: HirDialogueTransactionContext>(
        &self,
        owner: ExprId,
        inherited_role: SyntheticRole,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        self.validate_transaction_with_roles(owner, inherited_role, inherited_role, context)
    }

    fn validate_transaction_with_roles<C: HirDialogueTransactionContext>(
        &self,
        owner: ExprId,
        index_role: SyntheticRole,
        dialogue_role: SyntheticRole,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::Expression {
                id: self.target,
                expected: HirDialogueExpressionExpectation::Unrestricted,
            })
            .map_err(HirDialogueTransactionError::Context)?;
        if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = self.candidates {
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: index,
                    expected: HirDialogueExpressionExpectation::PostfixIndexCandidate {
                        owner,
                        role: index_role,
                        target: self.target,
                    },
                })
                .map_err(HirDialogueTransactionError::Context)?;
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: dialogue,
                    expected: HirDialogueExpressionExpectation::DialogueContentCandidate {
                        owner,
                        role: dialogue_role,
                        target: self.target,
                    },
                })
                .map_err(HirDialogueTransactionError::Context)?;
        }
        Ok(())
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(self.candidates, HirPostfixBracketCandidates::Invalid { .. })
    }
}

/// The exact two-result postfix carrier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPostfixBracketCandidates {
    Ambiguous {
        index: ExprId,
        dialogue: ExprId,
    },
    Invalid {
        index: HirPostfixCandidateFailure,
        dialogue: HirPostfixCandidateFailure,
    },
}

/// One bounded typed candidate failure.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPostfixCandidateFailure {
    kind: HirPostfixCandidateFailureKind,
}

impl HirPostfixCandidateFailure {
    pub(crate) const fn new(kind: HirPostfixCandidateFailureKind) -> Self {
        Self { kind }
    }

    /// Returns the grammar-owned failure family.
    pub const fn kind(&self) -> HirPostfixCandidateFailureKind {
        self.kind
    }
}

/// Grammar reasons for a failed postfix interpretation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueOrdinalError {
    Node { ordinal: usize },
    Argument { ordinal: usize },
    Mark { ordinal: usize },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueExpressionExpectation {
    Unrestricted,
    /// An expression produced by a `#` content application. This remains a
    /// dedicated transaction role even though content-root admission belongs
    /// to the later semantic layer.
    ContentApplication,
    Call,
    PostfixIndexCandidate {
        owner: ExprId,
        role: SyntheticRole,
        target: ExprId,
    },
    DialogueContentCandidate {
        owner: ExprId,
        role: SyntheticRole,
        target: ExprId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirRichTextCharge {
    PointActions { observed: usize },
    ContentArguments { observed: usize },
    ArgumentKeyBytes { observed: usize },
    ArgumentValueDecodedBytes { observed: usize },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueTransactionRequirement {
    Expression {
        id: ExprId,
        expected: HirDialogueExpressionExpectation,
    },
    Statement(StmtId),
    Scope(ScopeId),
    Type(TypeId),
    RichTextCharge(HirRichTextCharge),
}

pub(crate) trait HirDialogueTransactionContext {
    type Error;

    fn require(
        &mut self,
        requirement: HirDialogueTransactionRequirement,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueTransactionError<E> {
    Invariant(HirDialogueInvariantError),
    Context(E),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueInvariantError {
    ArithmeticOverflow,
    ForeignChild {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    InvalidArgumentReference,
    InvalidContentCallInvocation,
    InvalidContentCallSemanticEvidence,
    InvalidContentOwner,
    InvalidPostfixCandidate,
    InvalidMarkReference,
    NonContiguousMarkOrdinal,
    NonContiguousNodeOrdinal,
    DuplicateMarkName,
    MarkCatalogLimitExceeded {
        observed: usize,
        maximum: usize,
    },
    UnorderedCoordinates,
}

fn validate_coordinate_order(
    coordinates: &[HirDialogueCoordinate],
) -> Result<(), HirDialogueInvariantError> {
    if coordinates
        .windows(2)
        .any(|pair| pair[0].argument >= pair[1].argument)
    {
        Err(HirDialogueInvariantError::UnorderedCoordinates)
    } else {
        Ok(())
    }
}

fn validate_line_plan_items(
    expected: HirModuleId,
    items: &[HirLinePlanItem],
) -> Result<(), HirModuleId> {
    for item in items {
        match item {
            HirLinePlanItem::Init(statements) => {
                for statement in statements {
                    validate_module(expected, statement.module())?;
                }
            }
            HirLinePlanItem::Thread(statement)
            | HirLinePlanItem::On(statement)
            | HirLinePlanItem::Statement(statement)
            | HirLinePlanItem::CancelRule(statement)
            | HirLinePlanItem::Error(statement) => {
                validate_module(expected, statement.module())?;
            }
            HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                validate_line_plan_items(expected, items)?;
            }
        }
    }
    Ok(())
}

fn report_line_plan_items<C: HirDialogueTransactionContext>(
    items: &[HirLinePlanItem],
    context: &mut C,
) -> Result<(), HirDialogueTransactionError<C::Error>> {
    for item in items {
        match item {
            HirLinePlanItem::Init(statements) => {
                for statement in statements {
                    context
                        .require(HirDialogueTransactionRequirement::Statement(*statement))
                        .map_err(HirDialogueTransactionError::Context)?;
                }
            }
            HirLinePlanItem::Thread(statement)
            | HirLinePlanItem::On(statement)
            | HirLinePlanItem::Statement(statement)
            | HirLinePlanItem::CancelRule(statement)
            | HirLinePlanItem::Error(statement) => context
                .require(HirDialogueTransactionRequirement::Statement(*statement))
                .map_err(HirDialogueTransactionError::Context)?,
            HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                report_line_plan_items(items, context)?;
            }
        }
    }
    Ok(())
}

fn line_plan_items_have_recovery(items: &[HirLinePlanItem]) -> bool {
    items.iter().any(|item| match item {
        HirLinePlanItem::Error(_) => true,
        HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
            line_plan_items_have_recovery(items)
        }
        _ => false,
    })
}

fn validate_module(expected: HirModuleId, actual: HirModuleId) -> Result<(), HirModuleId> {
    if expected == actual {
        Ok(())
    } else {
        Err(actual)
    }
}

#[cfg(test)]
#[path = "dialogue_application/tests.rs"]
mod tests;
