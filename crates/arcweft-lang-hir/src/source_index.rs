//! Typed, revision-bound source components for final HIR owners.

mod block_projection;
mod call_cursor;
mod control_projection;
mod expr_projection;
mod expression_manifest;
mod flow_role;
mod item_projection;
mod item_role;
mod match_projection;
mod pattern_projection;
mod stmt_projection;
mod style_role;
mod thread_body_projection;
mod type_projection;
mod view_role;

use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_lang_syntax::attachment::{
    SyntaxAccessError, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotId,
};
use arcweft_lang_syntax::id_ref::SyntaxIdRefPart;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceRevision, SourceSpan,
};
use thiserror::Error;

use crate::expr::HirThreadBodyOwner;
use crate::expr::{HirCallArgumentOrdinal, HirCallTypeArgumentOrdinal};
use crate::identity::{
    ExprId, IdResolveError, ItemId, LocalId, PatternId, ScopeId, StmtId, SyntheticOwner,
    SyntheticRole, TypeId,
};
use crate::slot::{HirOrigin, HirSlotTransactionLease, SlotSnapshot, StagedSlotTransaction};

/// Failure to project one exact attached syntax identity into a typed HIR
/// arena owner.
///
/// The lookup is revision-bound through the module's retained syntax lineage.
/// It never reparses source text or derives an owner from source position.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirSourceLookupError {
    #[error("syntax ID belongs to database {actual:?}, expected {expected:?}")]
    WrongSyntaxDatabase {
        expected: SyntaxDatabaseId,
        actual: SyntaxDatabaseId,
    },
    #[error("syntax ID belongs to lineage {actual:?}, expected {expected:?}")]
    WrongSyntaxLineage {
        expected: SyntaxLineageId,
        actual: SyntaxLineageId,
    },
    #[error("syntax node {syntax:?} was not lowered as {expected:?}")]
    NotLowered {
        syntax: SyntaxNodeId,
        expected: crate::identity::HirIdKind,
    },
    #[error("syntax node {syntax:?} was lowered as {actual:?}, not requested {expected:?}")]
    KindMismatch {
        syntax: SyntaxNodeId,
        expected: crate::identity::HirIdKind,
        actual: crate::identity::HirIdKind,
    },
}

pub(crate) use expression_manifest::expression_component_role;
pub use flow_role::{
    HirFlowContractSourcePart, HirFlowParameterSourcePart, HirFlowReturnSourcePart,
    HirFlowSourceRole,
};
pub(crate) use item_projection::ItemValidationArenas;
pub use item_role::{
    HirCallableEffectSourcePart, HirCallableParameterSourcePart, HirCallableSourceOwner,
    HirCallableSourceRole, HirDeclarationSourceRole, HirEntrySourcePart, HirItemSourceRole,
    HirNominalMemberSourcePart, HirTestBenchSourceRole, HirUseBindingSourcePart, HirUseSourceRole,
};
pub use style_role::{
    HirStyleBodyPath, HirStyleBodySourcePart, HirStyleSourceRole, HirStyleTokenSourcePart,
};
pub use view_role::{HirViewBodySourcePart, HirViewExportSourcePart, HirViewSourceRole};

/// Revision-bound source span or checked zero-width insertion point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceSite {
    /// Authored source bytes.
    Span(SourceSpan),
    /// Parser-owned location for an omitted or recovered component.
    Insertion(HirInsertionPoint),
}

impl HirSourceSite {
    pub fn source_identity(&self) -> &SourceDocumentIdentity {
        match self {
            Self::Span(span) => span.source(),
            Self::Insertion(insertion) => insertion.source_identity(),
        }
    }

    /// Converts one parser-owned attached component without inspecting source
    /// text. Zero-width components retain their checked insertion identity;
    /// authored components retain the exact snapshot-bound span.
    pub(crate) fn from_attached_span(
        document: &SourceDocument,
        span: &SourceSpan,
    ) -> Result<Self, HirInsertionPointError> {
        if span.range().start() == span.range().end() {
            HirInsertionPoint::try_new(document, span.range().start()).map(Self::Insertion)
        } else {
            Ok(Self::Span(span.clone()))
        }
    }
}

/// Checked zero-width location in one exact source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInsertionPoint {
    source: SourceDocumentIdentity,
    offset: usize,
}

impl HirInsertionPoint {
    pub fn try_new(
        document: &SourceDocument,
        offset: usize,
    ) -> Result<Self, HirInsertionPointError> {
        let document_len = document.text().len();
        if offset > document_len {
            return Err(HirInsertionPointError::OutOfDocument {
                offset,
                document_len,
            });
        }
        if !document.text().is_char_boundary(offset) {
            return Err(HirInsertionPointError::NonUtf8Boundary { offset });
        }
        Ok(Self {
            source: document.identity().clone(),
            offset,
        })
    }

    pub const fn source_identity(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }
}

/// Invalid zero-width source insertion supplied by attached lowering.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirInsertionPointError {
    #[error("insertion offset {offset} is outside the {document_len}-byte source document")]
    OutOfDocument { offset: usize, document_len: usize },
    #[error("insertion offset {offset} is not a UTF-8 boundary")]
    NonUtf8Boundary { offset: usize },
}

/// Whole/name/value component of one ordinary call argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallArgumentSourcePart {
    Whole,
    Name,
    Equals,
    Value,
    Spread,
}

/// Whole/type component of one explicit Call type argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeArgumentSourcePart {
    Whole,
    Type,
}

/// Source component of the optional explicit type application on one Call.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeApplicationSourceRole {
    Whole,
    TurbofishSeparator,
    OpenAngle,
    CloseAngle,
    RecoveryEnd,
    EmptyInsertion,
    Argument {
        argument: HirCallTypeArgumentOrdinal,
        part: HirCallTypeArgumentSourcePart,
    },
    Separator {
        following: HirCallTypeArgumentOrdinal,
    },
    TrailingSeparator,
}

/// Source component of one record-expression field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecordFieldSourcePart {
    Whole,
    Name,
    Colon,
    Value,
}

/// Source component of one closure parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirClosureParameterSourcePart {
    Whole,
    Pattern,
    Colon,
    Type,
}

/// Source component of one match arm.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMatchArmSourcePart {
    Whole,
    Pattern,
    Guard,
    Arrow,
    Value,
}

/// Source component of one Dialogue content node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueNodeSourcePart {
    Whole,
    Text,
    Raw,
    Escape,
    RubyBase,
    RubyText,
    Interpolation,
    LineBreak,
    Error,
}

/// Source component of one `RichText` tag.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextTagSourcePart {
    Whole,
    OpenDelimiter,
    Name,
    Payload,
    CloseDelimiter,
    InferenceInsertion,
    EndTag,
    Marker(HirIdRefSourcePart),
}

/// Source component of one `RichText` argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextArgumentSourcePart {
    Whole,
    Name,
    Equals,
    Value,
}

/// Typed source component of one HIR expression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExprSourceRole {
    Whole,
    Target,
    OpenBracket,
    CloseBracket,
    Colon,
    Content,
    ContentBody,
    Plan,
    ConfigurationArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
    LiteralBody,
    LiteralPrefix,
    LiteralSuffix,
    LiteralUnit,
    EntityReference(HirIdRefSourcePart),
    PathRoot,
    PathSegment {
        ordinal: u32,
    },
    ShortVariantName,
    RegistryScope,
    RegistryKeySegment {
        ordinal: u32,
    },
    OptionalMarker,
    PlaceholderMarker,
    Element {
        ordinal: u32,
    },
    NumericElement {
        ordinal: u32,
    },
    NumericCommonSuffix,
    RepeatValue,
    RepeatLength,
    CallCallee,
    CallAssociatedReceiver,
    CallAssociatedSeparator,
    CallAssociatedMember,
    CallArgumentListOpen,
    CallArgumentListClose,
    CallArgumentListRecoveryEnd,
    CallArgumentListEmptyInsertion,
    CallArgumentSeparator {
        following: HirCallArgumentOrdinal,
    },
    CallArgumentTrailingSeparator,
    CallArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
    CallTypeApplication(HirCallTypeApplicationSourceRole),
    SelectedMember,
    Index,
    LeftOperand,
    RightOperand,
    Operand,
    Operator,
    RangeStart,
    RangeEnd,
    RangeInclusiveMarker,
    RecordPath,
    RecordField {
        field: u32,
        part: HirRecordFieldSourcePart,
    },
    ClosureParameter {
        parameter: u32,
        part: HirClosureParameterSourcePart,
    },
    ReturnType,
    Body,
    Statement {
        ordinal: u32,
    },
    Tail,
    Name,
    Condition,
    ThenBranch,
    ElseBranch,
    Pattern,
    Scrutinee,
    Guard,
    MatchArm {
        arm: u32,
        part: HirMatchArmSourcePart,
    },
    ThreadModifier,
    ThreadName,
    DialogueNode {
        ordinal: u32,
        part: HirDialogueNodeSourcePart,
    },
    RichTextTag {
        tag: u32,
        part: HirRichTextTagSourcePart,
    },
    RichTextArgument {
        tag: u32,
        argument: u16,
        part: HirRichTextArgumentSourcePart,
    },
    Recovery,
}

/// Exact source and ordinal descriptor for one statement-owned synthetic
/// `RecoveryOperand` expression.
///
/// Final lowering and source-freeze validation share this closed inventory so
/// neither side can independently reconstruct statement child ordinals.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirStmtRecoveryOperandSlot {
    LetInitializer { insertion: usize },
    AssignmentTarget { insertion: usize },
    AssignmentValue { insertion: usize },
    LifetimeSetTarget { insertion: usize },
    LifetimeSetValue { insertion: usize },
    OutValue { insertion: usize },
    GotoTarget { insertion: usize },
    DeferExpression { insertion: usize },
    SignalTarget { insertion: usize },
    SignalValue { insertion: usize },
    TriggerTarget { insertion: usize },
    TriggerExpression { insertion: usize },
    ReturnValue { insertion: usize },
    YieldExpression { insertion: usize },
    WaitTarget { insertion: usize },
    CloseTarget { insertion: usize },
    WhileCondition { insertion: usize },
    WhileLetScrutinee { insertion: usize },
    WhileLetGuard { insertion: usize },
    ForSource { insertion: usize },
    SelectOperand { insertion: usize },
    SelectBranchSource { insertion: usize, branch: u32 },
    MatchScrutinee { insertion: usize },
    MatchArmGuard { insertion: usize, arm: u32 },
    UnsafeAuditReason { insertion: usize },
}

impl HirStmtRecoveryOperandSlot {
    pub(crate) const fn insertion(self) -> usize {
        match self {
            Self::LetInitializer { insertion }
            | Self::AssignmentTarget { insertion }
            | Self::AssignmentValue { insertion }
            | Self::LifetimeSetTarget { insertion }
            | Self::LifetimeSetValue { insertion }
            | Self::OutValue { insertion }
            | Self::GotoTarget { insertion }
            | Self::DeferExpression { insertion }
            | Self::SignalTarget { insertion }
            | Self::SignalValue { insertion }
            | Self::TriggerTarget { insertion }
            | Self::TriggerExpression { insertion }
            | Self::ReturnValue { insertion }
            | Self::YieldExpression { insertion }
            | Self::WaitTarget { insertion }
            | Self::CloseTarget { insertion }
            | Self::WhileCondition { insertion }
            | Self::WhileLetScrutinee { insertion }
            | Self::WhileLetGuard { insertion }
            | Self::ForSource { insertion }
            | Self::SelectOperand { insertion }
            | Self::SelectBranchSource { insertion, .. }
            | Self::MatchScrutinee { insertion }
            | Self::MatchArmGuard { insertion, .. }
            | Self::UnsafeAuditReason { insertion } => insertion,
        }
    }

    pub(crate) const fn ordinal(self) -> Option<u32> {
        match self {
            Self::AssignmentValue { .. }
            | Self::LifetimeSetValue { .. }
            | Self::SignalValue { .. }
            | Self::WhileLetGuard { .. } => Some(1),
            Self::SelectBranchSource { branch, .. } => Some(branch),
            Self::MatchArmGuard { arm, .. } => arm.checked_add(1),
            Self::LetInitializer { .. }
            | Self::AssignmentTarget { .. }
            | Self::LifetimeSetTarget { .. }
            | Self::OutValue { .. }
            | Self::GotoTarget { .. }
            | Self::DeferExpression { .. }
            | Self::SignalTarget { .. }
            | Self::TriggerTarget { .. }
            | Self::TriggerExpression { .. }
            | Self::ReturnValue { .. }
            | Self::YieldExpression { .. }
            | Self::WaitTarget { .. }
            | Self::CloseTarget { .. }
            | Self::WhileCondition { .. }
            | Self::WhileLetScrutinee { .. }
            | Self::ForSource { .. }
            | Self::SelectOperand { .. }
            | Self::MatchScrutinee { .. }
            | Self::UnsafeAuditReason { .. } => Some(0),
        }
    }

    pub(crate) const fn source_role(self) -> HirExprSourceRole {
        match self {
            Self::AssignmentTarget { .. }
            | Self::LifetimeSetTarget { .. }
            | Self::GotoTarget { .. }
            | Self::SignalTarget { .. }
            | Self::TriggerTarget { .. }
            | Self::WaitTarget { .. }
            | Self::CloseTarget { .. } => HirExprSourceRole::Target,
            Self::WhileCondition { .. } => HirExprSourceRole::Condition,
            Self::WhileLetScrutinee { .. }
            | Self::ForSource { .. }
            | Self::MatchScrutinee { .. } => HirExprSourceRole::Scrutinee,
            Self::WhileLetGuard { .. } | Self::MatchArmGuard { .. } => HirExprSourceRole::Guard,
            Self::LetInitializer { .. }
            | Self::AssignmentValue { .. }
            | Self::LifetimeSetValue { .. }
            | Self::OutValue { .. }
            | Self::DeferExpression { .. }
            | Self::SignalValue { .. }
            | Self::ReturnValue { .. }
            | Self::YieldExpression { .. }
            | Self::SelectOperand { .. }
            | Self::SelectBranchSource { .. }
            | Self::TriggerExpression { .. }
            | Self::UnsafeAuditReason { .. } => HirExprSourceRole::Operand,
        }
    }
}

/// Source component shared by every literal family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLiteralSourcePart {
    Body,
    Prefix,
    Suffix,
    Unit,
}

/// Source component of a structured Arcweft ID reference.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdRefSourcePart {
    Whole,
    AbsoluteMarker,
    Family,
    FamilySeparator,
    ParentMarker { ordinal: u32 },
    SuffixSegment { ordinal: u32 },
}

impl From<SyntaxIdRefPart> for HirIdRefSourcePart {
    fn from(value: SyntaxIdRefPart) -> Self {
        match value {
            SyntaxIdRefPart::Whole => Self::Whole,
            SyntaxIdRefPart::AbsoluteMarker => Self::AbsoluteMarker,
            SyntaxIdRefPart::Family => Self::Family,
            SyntaxIdRefPart::FamilySeparator => Self::FamilySeparator,
            SyntaxIdRefPart::ParentMarker { ordinal } => Self::ParentMarker { ordinal },
            SyntaxIdRefPart::SuffixSegment { ordinal } => Self::SuffixSegment { ordinal },
        }
    }
}

/// Source component of a qualified or shorthand variant-pattern head.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternHeadSourcePart {
    QualifiedRoot,
    QualifiedSegment { ordinal: u32 },
    DotShorthandMarker,
}

/// Source component of an optional variant-pattern payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternPayloadSourcePart {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
}

/// Source component of one record-pattern field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternFieldSourcePart {
    Whole,
    Name,
    Colon,
    Pattern,
    RestMarker,
    RestBinding,
}

/// Source component of a bracket-pattern rest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternRestSourcePart {
    Whole,
    Marker,
    Binding,
}

/// Typed source component of one HIR pattern.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternSourceRole {
    Whole,
    Name,
    MutKeyword,
    Literal(HirLiteralSourcePart),
    EntityReference(HirIdRefSourcePart),
    VariantHead(HirVariantPatternHeadSourcePart),
    VariantName,
    VariantPayload(HirVariantPatternPayloadSourcePart),
    Element {
        ordinal: u32,
    },
    RecordPathRoot,
    RecordPathSegment {
        ordinal: u32,
    },
    PatternField {
        field: u32,
        part: HirPatternFieldSourcePart,
    },
    SequenceRest(HirPatternRestSourcePart),
    WholeBindingName,
    NestedPattern,
    TypedBindingColon,
    TypedBindingType,
    Recovery,
}

/// Source component of one associated-type binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAssociatedTypeBindingSourcePart {
    Whole,
    Name,
    Equals,
    Value,
}

/// Source component of a named or elided type region.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTypeRegionSourcePart {
    Whole,
    NamedApostrophe,
    NamedName,
    ElisionInsertion,
}

/// Typed source component of one HIR type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTypeSourceRole {
    Whole,
    NeverMarker,
    ConstInteger,
    PathRoot,
    PathSegment {
        ordinal: u32,
    },
    TupleOpen,
    TupleElement {
        ordinal: u32,
    },
    TupleSeparator {
        ordinal: u32,
    },
    TupleClose,
    FunctionOpen,
    FunctionParameter {
        ordinal: u32,
    },
    FunctionSeparator {
        ordinal: u32,
    },
    FunctionClose,
    FunctionArrow,
    FunctionReturn,
    FunctionEffectOpen,
    FunctionEffect {
        ordinal: u32,
    },
    FunctionEffectClose,
    ChoiceAlternative {
        ordinal: u32,
    },
    ChoiceSeparator {
        ordinal: u32,
    },
    GenericBase,
    GenericOpen,
    GenericArgument {
        ordinal: u32,
    },
    GenericSeparator {
        ordinal: u32,
    },
    GenericClose,
    TraitBase,
    TraitOpen,
    TraitArgument {
        ordinal: u32,
    },
    TraitSeparator {
        ordinal: u32,
    },
    AssociatedBinding {
        ordinal: u32,
        part: HirAssociatedTypeBindingSourcePart,
    },
    TraitClose,
    ProjectionSubject,
    ProjectionSeparator,
    ProjectionName,
    ReferenceAmpersand,
    Region(HirTypeRegionSourcePart),
    ReferenceMutKeyword,
    ReferenceReferent,
    SliceOpen,
    SliceElement,
    SliceClose,
    Recovery,
}

/// Typed source component of one HIR statement.
///
/// Statement source ownership is intentionally narrow. Statement payloads do
/// not retain raw ranges; the unsafe-audit edit anchor is published through
/// the same revision-bound component index as every other final-HIR source
/// component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtSourceRole {
    Whole,
    UnsafeAuditInsertion,
}

/// Typed source component of one lexical scope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirScopeSourceRole {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
    SyntheticOrigin,
}

/// Typed source component of one local binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalSourceRole {
    Whole,
    Name,
    Type,
    Pattern,
    SyntheticOrigin,
}

/// Source component selected inside one shared Flow/Thread body row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadFlowItemSourcePart {
    Whole,
    ChildWhole,
}

/// Typed source component of one shared Flow/Thread body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadBodySourceRole {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
    Item {
        ordinal: u32,
        part: HirThreadFlowItemSourcePart,
    },
}

/// One typed HIR owner paired with only its applicable source-role family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceQuery {
    Item {
        owner: ItemId,
        role: HirItemSourceRole,
    },
    Expr {
        owner: ExprId,
        role: HirExprSourceRole,
    },
    Pattern {
        owner: PatternId,
        role: HirPatternSourceRole,
    },
    Type {
        owner: TypeId,
        role: HirTypeSourceRole,
    },
    Stmt {
        owner: StmtId,
        role: HirStmtSourceRole,
    },
    Scope {
        owner: ScopeId,
        role: HirScopeSourceRole,
    },
    Local {
        owner: LocalId,
        role: HirLocalSourceRole,
    },
    ThreadBody {
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
    },
}

impl HirSourceQuery {
    /// Returns whether `Whole` is retained by the raw owner slot rather than
    /// by the component table.
    pub const fn is_slot_whole(&self) -> bool {
        matches!(
            self,
            Self::Item {
                role: HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole)
                    | HirItemSourceRole::Entry(HirEntrySourcePart::Whole)
                    | HirItemSourceRole::Use(HirUseSourceRole::Whole)
                    | HirItemSourceRole::TestBench(HirTestBenchSourceRole::Whole)
                    | HirItemSourceRole::Flow(HirFlowSourceRole::Whole)
                    | HirItemSourceRole::View(HirViewSourceRole::Whole),
                ..
            } | Self::Expr {
                role: HirExprSourceRole::Whole,
                ..
            } | Self::Pattern {
                role: HirPatternSourceRole::Whole,
                ..
            } | Self::Type {
                role: HirTypeSourceRole::Whole,
                ..
            } | Self::Stmt {
                role: HirStmtSourceRole::Whole,
                ..
            } | Self::Scope {
                role: HirScopeSourceRole::Whole,
                ..
            } | Self::Local {
                role: HirLocalSourceRole::Whole,
                ..
            }
        )
    }

    pub const fn owner(&self) -> SyntheticOwner {
        match self {
            Self::Item { owner, .. } => SyntheticOwner::Item(*owner),
            Self::Expr { owner, .. } => SyntheticOwner::Expr(*owner),
            Self::Pattern { owner, .. } => SyntheticOwner::Pattern(*owner),
            Self::Type { owner, .. } => SyntheticOwner::Type(*owner),
            Self::Stmt { owner, .. } => SyntheticOwner::Stmt(*owner),
            Self::Scope { owner, .. } => SyntheticOwner::Scope(*owner),
            Self::Local { owner, .. } => SyntheticOwner::Local(*owner),
            Self::ThreadBody { owner, .. } => match owner {
                HirThreadBodyOwner::Flow(owner) => SyntheticOwner::Item(*owner),
                HirThreadBodyOwner::ThreadExpression(owner) => SyntheticOwner::Expr(*owner),
                HirThreadBodyOwner::NestedScope(owner) => SyntheticOwner::Scope(*owner),
            },
        }
    }
}

/// Source-site presence after typed owner and role validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSourcePresence<'a> {
    Present(&'a HirSourceSite),
    AbsentOptional,
}

/// Executability status read from the immutable owner slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceOwnerStatus {
    Clean,
    Poisoned,
}

/// Exact source-site result for one typed HIR query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSourceLookup<'a> {
    presence: HirSourcePresence<'a>,
    owner_status: HirSourceOwnerStatus,
}

impl<'a> HirSourceLookup<'a> {
    const fn new(presence: HirSourcePresence<'a>, owner_status: HirSourceOwnerStatus) -> Self {
        Self {
            presence,
            owner_status,
        }
    }

    pub const fn presence(&self) -> HirSourcePresence<'a> {
        self.presence
    }

    pub const fn owner_status(&self) -> HirSourceOwnerStatus {
        self.owner_status
    }
}

/// Typed failure from the sole final HIR source query.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSourceQueryError {
    #[error("failed to resolve item source owner {owner:?}")]
    ItemResolve {
        owner: ItemId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve expression source owner {owner:?}")]
    ExprResolve {
        owner: ExprId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve pattern source owner {owner:?}")]
    PatternResolve {
        owner: PatternId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve type source owner {owner:?}")]
    TypeResolve {
        owner: TypeId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve statement source owner {owner:?}")]
    StmtResolve {
        owner: StmtId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve scope source owner {owner:?}")]
    ScopeResolve {
        owner: ScopeId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve local source owner {owner:?}")]
    LocalResolve {
        owner: LocalId,
        #[source]
        error: IdResolveError,
    },
    #[error("failed to resolve Thread-body source owner {owner:?}")]
    ThreadBodyResolve {
        owner: HirThreadBodyOwner,
        #[source]
        error: IdResolveError,
    },
    #[error("source role {role:?} is not applicable to item {owner:?}")]
    ItemRoleNotApplicable {
        owner: ItemId,
        role: HirItemSourceRole,
    },
    #[error("source role {role:?} is not applicable to expression {owner:?}")]
    ExprRoleNotApplicable {
        owner: ExprId,
        role: HirExprSourceRole,
    },
    #[error("source role {role:?} is not applicable to pattern {owner:?}")]
    PatternRoleNotApplicable {
        owner: PatternId,
        role: HirPatternSourceRole,
    },
    #[error("source role {role:?} is not applicable to type {owner:?}")]
    TypeRoleNotApplicable {
        owner: TypeId,
        role: HirTypeSourceRole,
    },
    #[error("source role {role:?} is not applicable to statement {owner:?}")]
    StmtRoleNotApplicable {
        owner: StmtId,
        role: HirStmtSourceRole,
    },
    #[error("source role {role:?} is not applicable to scope {owner:?}")]
    ScopeRoleNotApplicable {
        owner: ScopeId,
        role: HirScopeSourceRole,
    },
    #[error("source role {role:?} is not applicable to local {owner:?}")]
    LocalRoleNotApplicable {
        owner: LocalId,
        role: HirLocalSourceRole,
    },
    #[error("source role {role:?} is not applicable to Thread body {owner:?}")]
    ThreadBodyRoleNotApplicable {
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
    },
    #[error("source role {role:?} is outside expression {owner:?}'s length {length}")]
    ExprOrdinalOutOfBounds {
        owner: ExprId,
        role: HirExprSourceRole,
        length: u32,
    },
    #[error("source role {role:?} is outside item {owner:?}'s length {length}")]
    ItemOrdinalOutOfBounds {
        owner: ItemId,
        role: HirItemSourceRole,
        length: u32,
    },
    #[error("source role {role:?} is outside pattern {owner:?}'s length {length}")]
    PatternOrdinalOutOfBounds {
        owner: PatternId,
        role: HirPatternSourceRole,
        length: u32,
    },
    #[error("source role {role:?} is outside type {owner:?}'s length {length}")]
    TypeOrdinalOutOfBounds {
        owner: TypeId,
        role: HirTypeSourceRole,
        length: u32,
    },
    #[error("source role {role:?} is outside Thread body {owner:?}'s length {length}")]
    ThreadBodyOrdinalOutOfBounds {
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
        length: u32,
    },
    #[error("source query supplied document {actual}, expected {expected}")]
    WrongSourceDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    #[error("source query supplied revision {actual:?}, expected {expected:?}")]
    StaleSourceRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
    #[error("source query supplied length {actual}, expected {expected}")]
    SourceLengthMismatch { expected: u64, actual: u64 },
}

impl HirSourceQueryError {
    pub(crate) fn resolve(query: &HirSourceQuery, error: IdResolveError) -> Self {
        match query {
            HirSourceQuery::Item { owner, .. } => Self::ItemResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::Expr { owner, .. } => Self::ExprResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::Pattern { owner, .. } => Self::PatternResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::Type { owner, .. } => Self::TypeResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::Stmt { owner, .. } => Self::StmtResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::Scope { owner, .. } => Self::ScopeResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::Local { owner, .. } => Self::LocalResolve {
                owner: *owner,
                error,
            },
            HirSourceQuery::ThreadBody { owner, .. } => Self::ThreadBodyResolve {
                owner: *owner,
                error,
            },
        }
    }

    pub(crate) fn role_not_applicable(query: &HirSourceQuery) -> Self {
        match query {
            HirSourceQuery::Item { owner, role } => Self::ItemRoleNotApplicable {
                owner: *owner,
                role: role.clone(),
            },
            HirSourceQuery::Expr { owner, role } => Self::ExprRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
            HirSourceQuery::Pattern { owner, role } => Self::PatternRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
            HirSourceQuery::Type { owner, role } => Self::TypeRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
            HirSourceQuery::Stmt { owner, role } => Self::StmtRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
            HirSourceQuery::Scope { owner, role } => Self::ScopeRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
            HirSourceQuery::Local { owner, role } => Self::LocalRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
            HirSourceQuery::ThreadBody { owner, role } => Self::ThreadBodyRoleNotApplicable {
                owner: *owner,
                role: *role,
            },
        }
    }
}

/// Requiredness derived directly from one resolved HIR payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirSourceRequirement {
    Required,
    Optional,
}

/// Role validation result produced after resolving the typed owner payload.
pub(crate) enum HirResolvedSourceRole<'a> {
    Whole {
        site: &'a HirSourceSite,
        owner_status: HirSourceOwnerStatus,
    },
    Component {
        requirement: HirSourceRequirement,
        owner_status: HirSourceOwnerStatus,
    },
    /// A typed relation to another already-authoritative whole/component site.
    ///
    /// This is used by Scope, Local, and shared Thread-body roles so the source
    /// index does not copy child ranges into parallel component rows.
    Related {
        presence: HirSourcePresence<'a>,
        owner_status: HirSourceOwnerStatus,
    },
}

impl<'a> HirResolvedSourceRole<'a> {
    pub(crate) const fn whole(site: &'a HirSourceSite, owner_status: HirSourceOwnerStatus) -> Self {
        Self::Whole { site, owner_status }
    }

    pub(crate) const fn component(
        requirement: HirSourceRequirement,
        owner_status: HirSourceOwnerStatus,
    ) -> Self {
        Self::Component {
            requirement,
            owner_status,
        }
    }

    pub(crate) const fn related(
        presence: HirSourcePresence<'a>,
        owner_status: HirSourceOwnerStatus,
    ) -> Self {
        Self::Related {
            presence,
            owner_status,
        }
    }
}

/// Immutable non-`Whole` source-component table for one HIR snapshot.
#[derive(Clone)]
pub(crate) struct HirSourceIndex {
    source: SourceDocumentIdentity,
    transaction: Arc<HirSlotTransactionLease>,
    syntax_owners: Arc<BTreeMap<SyntheticOwner, SyntaxNodeId>>,
    requirements: Arc<BTreeMap<HirSourceQuery, HirSourceRequirement>>,
    components: Arc<BTreeMap<HirSourceQuery, HirSourceSite>>,
}

impl HirSourceIndex {
    #[cfg(test)]
    pub(crate) fn empty(source: SourceDocumentIdentity, slots: &SlotSnapshot) -> Self {
        Self {
            source,
            transaction: Arc::clone(slots.transaction_lease()),
            syntax_owners: Arc::new(BTreeMap::new()),
            requirements: Arc::new(BTreeMap::new()),
            components: Arc::new(BTreeMap::new()),
        }
    }

    /// Validates that every retained component belongs to this exact source
    /// revision and to a live owner in the unpublished slot proposal.
    pub(crate) fn validates_prepared(
        &self,
        slots: &SlotSnapshot,
        source: &SourceDocumentIdentity,
    ) -> bool {
        self.source == *source
            && Arc::ptr_eq(&self.transaction, slots.transaction_lease())
            && self.requirements.iter().all(|(query, requirement)| {
                !query.is_slot_whole()
                    && query_owner_is_prepared(query, &self.syntax_owners, slots)
                    && (*requirement == HirSourceRequirement::Optional
                        || self.components.contains_key(query))
            })
            && self
                .components
                .keys()
                .all(|query| self.requirements.contains_key(query))
            && self.components.iter().all(|(query, site)| {
                !query.is_slot_whole()
                    && validate_component_source(source, site.source_identity()).is_ok()
                    && query_owner_is_prepared(query, &self.syntax_owners, slots)
            })
            && self
                .syntax_owners
                .iter()
                .all(|(owner, syntax)| source_owner_is_prepared_at(*owner, *syntax, slots))
    }

    pub(crate) fn requirement(&self, query: &HirSourceQuery) -> Option<HirSourceRequirement> {
        self.requirements.get(query).copied()
    }

    /// Borrows a previously frozen typed component for another query role.
    ///
    /// Relational roles use this instead of copying the same source site under
    /// a second key. `None` means that the target role is not part of the
    /// committed manifest; an optional target without a site remains an
    /// explicit [`HirSourcePresence::AbsentOptional`].
    pub(crate) fn component_presence(
        &self,
        query: &HirSourceQuery,
    ) -> Option<HirSourcePresence<'_>> {
        match (self.requirements.get(query), self.components.get(query)) {
            (Some(_), Some(site)) => Some(HirSourcePresence::Present(site)),
            (Some(HirSourceRequirement::Optional), None) => Some(HirSourcePresence::AbsentOptional),
            (Some(HirSourceRequirement::Required), None) | (None, _) => None,
        }
    }

    /// Performs query work after the module supplies typed owner/payload
    /// resolution. Calling the resolver first preserves the normative ordering
    /// ahead of source identity checks.
    #[allow(
        clippy::result_large_err,
        reason = "lookup failures retain the complete typed query, source identities, and invariant evidence"
    )]
    pub(crate) fn lookup<'a>(
        &'a self,
        retained_source: &SourceDocumentIdentity,
        expected_source: &SourceDocumentIdentity,
        query: &HirSourceQuery,
        resolve_role: impl FnOnce(
            &HirSourceQuery,
        ) -> Result<HirResolvedSourceRole<'a>, HirSourceQueryError>,
    ) -> Result<HirSourceLookup<'a>, HirSourceIndexLookupError> {
        let resolved = resolve_role(query).map_err(HirSourceIndexLookupError::Query)?;
        validate_expected_source(retained_source, expected_source)
            .map_err(HirSourceIndexLookupError::Query)?;

        match resolved {
            HirResolvedSourceRole::Whole { site, owner_status } => {
                if !query.is_slot_whole() {
                    return Err(HirSourceCommitInvariantError::WholeResolutionMismatch {
                        query: query.clone(),
                    }
                    .into());
                }
                validate_component_source(retained_source, site.source_identity())?;
                Ok(HirSourceLookup::new(
                    HirSourcePresence::Present(site),
                    owner_status,
                ))
            }
            HirResolvedSourceRole::Component {
                requirement,
                owner_status,
            } => {
                if query.is_slot_whole() {
                    return Err(HirSourceCommitInvariantError::WholeResolutionMismatch {
                        query: query.clone(),
                    }
                    .into());
                }
                if let Some(site) = self.components.get(query) {
                    validate_component_source(retained_source, site.source_identity())?;
                    return Ok(HirSourceLookup::new(
                        HirSourcePresence::Present(site),
                        owner_status,
                    ));
                }
                if requirement == HirSourceRequirement::Optional {
                    return Ok(HirSourceLookup::new(
                        HirSourcePresence::AbsentOptional,
                        owner_status,
                    ));
                }
                Err(HirSourceCommitInvariantError::MissingRequiredComponent {
                    query: query.clone(),
                }
                .into())
            }
            HirResolvedSourceRole::Related {
                presence,
                owner_status,
            } => {
                if let HirSourcePresence::Present(site) = presence {
                    validate_component_source(retained_source, site.source_identity())?;
                }
                Ok(HirSourceLookup::new(presence, owner_status))
            }
        }
    }

    #[cfg(test)]
    fn component_count(&self) -> usize {
        self.components.len()
    }
}

fn validate_expected_source(
    retained: &SourceDocumentIdentity,
    supplied: &SourceDocumentIdentity,
) -> Result<(), HirSourceQueryError> {
    if retained.id() != supplied.id() {
        return Err(HirSourceQueryError::WrongSourceDocument {
            expected: retained.id().clone(),
            actual: supplied.id().clone(),
        });
    }
    if retained.revision() != supplied.revision() {
        return Err(HirSourceQueryError::StaleSourceRevision {
            expected: retained.revision(),
            actual: supplied.revision(),
        });
    }
    if retained.source_len() != supplied.source_len() {
        return Err(HirSourceQueryError::SourceLengthMismatch {
            expected: retained.source_len(),
            actual: supplied.source_len(),
        });
    }
    Ok(())
}

/// Internal lookup failure kept separate from the exact public query errors.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirSourceIndexLookupError {
    #[error(transparent)]
    Query(#[from] HirSourceQueryError),
    #[error(transparent)]
    Invariant(#[from] HirSourceCommitInvariantError),
}

/// Source-table publication invariant checked before an immutable snapshot is built.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirSourceCommitInvariantError {
    #[error("the Whole source role must remain owned by slot metadata: {query:?}")]
    WholeComponent { query: HirSourceQuery },
    #[error("source role has conflicting requiredness in one transaction: {query:?}")]
    ConflictingRequirement { query: HirSourceQuery },
    #[error("source component has conflicting sites in one transaction: {query:?}")]
    ConflictingComponent { query: HirSourceQuery },
    #[error("source component was staged without payload applicability: {query:?}")]
    UndeclaredComponent { query: HirSourceQuery },
    #[error("required source component was not staged: {query:?}")]
    MissingRequiredComponent { query: HirSourceQuery },
    #[error("source role resolution disagrees with Whole ownership: {query:?}")]
    WholeResolutionMismatch { query: HirSourceQuery },
    #[error("source component belongs to document {actual}, expected {expected}")]
    WrongSourceDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    #[error("source component belongs to revision {actual:?}, expected {expected:?}")]
    StaleSourceRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
    #[error("source component has source length {actual}, expected {expected}")]
    SourceLengthMismatch { expected: u64, actual: u64 },
    #[error("attached source owner {owner:?} was rebound from {existing:?} to {actual:?}")]
    ConflictingSyntaxOwner {
        owner: SyntheticOwner,
        existing: SyntaxNodeId,
        actual: SyntaxNodeId,
    },
    #[error("attached source node belongs to syntax snapshot {actual:?}, expected {expected:?}")]
    WrongSyntaxSnapshot {
        expected: SyntaxSnapshotId,
        actual: SyntaxSnapshotId,
    },
    #[error("attached syntax for HIR owner {owner:?} does not match its semantic payload family")]
    AttachedPayloadFamilyMismatch { owner: SyntheticOwner },
    #[error("failed to inspect attached syntax for HIR owner {owner:?}")]
    AttachedSyntaxAccess {
        owner: SyntheticOwner,
        #[source]
        error: SyntaxAccessError,
    },
    #[error("attached syntax for HIR owner {owner:?} does not match its semantic poison state")]
    AttachedPayloadStateMismatch { owner: SyntheticOwner },
    #[error(transparent)]
    InvalidInsertionPoint(#[from] HirInsertionPointError),
    #[error("source-index transaction is poisoned by an earlier failure")]
    TransactionPoisoned,
}

/// Mutable all-or-nothing source-component staging area.
pub(crate) struct StagedHirSourceIndex {
    source: SourceDocumentIdentity,
    transaction: Arc<HirSlotTransactionLease>,
    syntax_owners: BTreeMap<SyntheticOwner, SyntaxNodeId>,
    requirements: BTreeMap<HirSourceQuery, HirSourceRequirement>,
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
    poisoned: bool,
}

impl StagedHirSourceIndex {
    pub(crate) fn new(source: SourceDocumentIdentity, slots: &StagedSlotTransaction) -> Self {
        Self {
            source,
            transaction: Arc::clone(slots.transaction_lease()),
            syntax_owners: BTreeMap::new(),
            requirements: BTreeMap::new(),
            components: BTreeMap::new(),
            poisoned: false,
        }
    }

    /// Read-only source-role projection used by the unpublished project header
    /// barrier. It exposes only already-staged typed components and cannot
    /// commit, infer, or synthesize a missing role.
    pub(crate) fn component_presence(
        &self,
        query: &HirSourceQuery,
    ) -> Option<HirSourcePresence<'_>> {
        match (self.requirements.get(query), self.components.get(query)) {
            (Some(_), Some(site)) => Some(HirSourcePresence::Present(site)),
            (Some(HirSourceRequirement::Optional), None) => Some(HirSourcePresence::AbsentOptional),
            (Some(HirSourceRequirement::Required), None) | (None, _) => None,
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "transaction failures retain the complete conflicting typed owner evidence"
    )]
    fn bind_syntax_owner(
        &mut self,
        owner: SyntheticOwner,
        syntax: SyntaxNodeId,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        match self.syntax_owners.get(&owner).copied() {
            Some(existing) if existing != syntax => {
                self.reject(HirSourceCommitInvariantError::ConflictingSyntaxOwner {
                    owner,
                    existing,
                    actual: syntax,
                })
            }
            Some(_) => Ok(()),
            None => {
                self.syntax_owners.insert(owner, syntax);
                Ok(())
            }
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "transaction failures retain the complete conflicting typed requirement evidence"
    )]
    fn require(
        &mut self,
        query: &HirSourceQuery,
        requirement: HirSourceRequirement,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if query.is_slot_whole() {
            return self.reject(HirSourceCommitInvariantError::WholeComponent {
                query: query.clone(),
            });
        }
        match self.requirements.get(query) {
            Some(existing) if *existing != requirement => {
                self.reject(HirSourceCommitInvariantError::ConflictingRequirement {
                    query: query.clone(),
                })
            }
            Some(_) => Ok(()),
            None => {
                self.requirements.insert(query.clone(), requirement);
                Ok(())
            }
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "transaction failures retain the complete typed query and source-site evidence"
    )]
    fn stage(
        &mut self,
        query: &HirSourceQuery,
        site: HirSourceSite,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if query.is_slot_whole() {
            return self.reject(HirSourceCommitInvariantError::WholeComponent {
                query: query.clone(),
            });
        }
        if let Err(error) = validate_component_source(&self.source, site.source_identity()) {
            return self.reject(error);
        }
        match self.components.get(query) {
            Some(existing) if existing != &site => {
                self.reject(HirSourceCommitInvariantError::ConflictingComponent {
                    query: query.clone(),
                })
            }
            Some(_) => Ok(()),
            None => {
                self.components.insert(query.clone(), site);
                Ok(())
            }
        }
    }

    /// Test-only mutation hook for proving that publication rejects an
    /// incomplete exact manifest. This is absent from production builds.
    #[cfg(test)]
    pub(crate) fn remove_staged_query(&mut self, query: &HirSourceQuery) -> bool {
        self.requirements.remove(query).is_some() && self.components.remove(query).is_some()
    }

    /// Test-only mutation hook for proving that publication rejects an extra
    /// optional manifest row. This is absent from production builds.
    #[cfg(test)]
    #[allow(
        clippy::result_large_err,
        reason = "the test hook preserves the same structured transaction error as production staging"
    )]
    pub(crate) fn stage_absent_optional_query(
        &mut self,
        query: &HirSourceQuery,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.require(query, HirSourceRequirement::Optional)
    }

    #[cfg(test)]
    #[allow(
        clippy::result_large_err,
        reason = "the test hook preserves the same structured transaction error as production staging"
    )]
    pub(crate) fn inject_component_for_test(
        &mut self,
        query: &HirSourceQuery,
        site: HirSourceSite,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.stage(query, site)
    }

    #[allow(
        clippy::result_large_err,
        reason = "commit rejection preserves the complete missing or undeclared typed manifest row"
    )]
    pub(crate) fn commit(self) -> Result<HirSourceIndex, HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if let Some(query) = self
            .components
            .keys()
            .find(|query| !self.requirements.contains_key(query))
            .cloned()
        {
            return Err(HirSourceCommitInvariantError::UndeclaredComponent { query });
        }
        if let Some(query) = self
            .requirements
            .iter()
            .find(|(query, requirement)| {
                **requirement == HirSourceRequirement::Required
                    && !self.components.contains_key(query)
            })
            .map(|(query, _)| query.clone())
        {
            return Err(HirSourceCommitInvariantError::MissingRequiredComponent { query });
        }

        Ok(HirSourceIndex {
            source: self.source,
            transaction: self.transaction,
            syntax_owners: Arc::new(self.syntax_owners),
            requirements: Arc::new(self.requirements),
            components: Arc::new(self.components),
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "the transaction uses one structured invariant error family at every fallible boundary"
    )]
    const fn ensure_open(&self) -> Result<(), HirSourceCommitInvariantError> {
        if self.poisoned {
            Err(HirSourceCommitInvariantError::TransactionPoisoned)
        } else {
            Ok(())
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "rejection returns the original structured invariant error after poisoning the transaction"
    )]
    fn reject<T>(
        &mut self,
        error: HirSourceCommitInvariantError,
    ) -> Result<T, HirSourceCommitInvariantError> {
        self.poisoned = true;
        Err(error)
    }
}

fn query_owner_is_prepared(
    query: &HirSourceQuery,
    syntax_owners: &BTreeMap<SyntheticOwner, SyntaxNodeId>,
    slots: &SlotSnapshot,
) -> bool {
    match query {
        HirSourceQuery::Item { owner, .. } => slots
            .resolve_prepared(*owner)
            .is_ok_and(|metadata| matches!(metadata.origin(), HirOrigin::Source(_))),
        HirSourceQuery::Expr { owner, .. } => {
            expression_query_owner_is_prepared(*owner, syntax_owners, slots)
        }
        HirSourceQuery::Pattern { owner, .. } => syntax_owners
            .get(&SyntheticOwner::Pattern(*owner))
            .is_some_and(|syntax| {
                prepared_source_origin_matches(slots.resolve_prepared(*owner), *syntax)
            }),
        HirSourceQuery::Type { owner, .. } => syntax_owners
            .get(&SyntheticOwner::Type(*owner))
            .is_some_and(|syntax| {
                prepared_source_origin_matches(slots.resolve_prepared(*owner), *syntax)
            }),
        HirSourceQuery::Stmt { owner, .. } => syntax_owners
            .get(&SyntheticOwner::Stmt(*owner))
            .is_some_and(|syntax| {
                prepared_source_origin_matches(slots.resolve_prepared(*owner), *syntax)
            }),
        HirSourceQuery::Scope { owner, .. } => slots.resolve_prepared(*owner).is_ok(),
        HirSourceQuery::Local { owner, .. } => slots.resolve_prepared(*owner).is_ok(),
        HirSourceQuery::ThreadBody { owner, .. } => match owner {
            HirThreadBodyOwner::Flow(owner) => slots
                .resolve_prepared(*owner)
                .is_ok_and(|metadata| matches!(metadata.origin(), HirOrigin::Source(_))),
            HirThreadBodyOwner::ThreadExpression(owner) => syntax_owners
                .get(&SyntheticOwner::Expr(*owner))
                .is_some_and(|syntax| {
                    prepared_source_origin_matches(slots.resolve_prepared(*owner), *syntax)
                }),
            HirThreadBodyOwner::NestedScope(owner) => slots
                .resolve_prepared(*owner)
                .is_ok_and(|metadata| matches!(metadata.origin(), HirOrigin::Source(_))),
        },
    }
}

fn expression_query_owner_is_prepared(
    owner: ExprId,
    syntax_owners: &BTreeMap<SyntheticOwner, SyntaxNodeId>,
    slots: &SlotSnapshot,
) -> bool {
    if syntax_owners
        .get(&SyntheticOwner::Expr(owner))
        .is_some_and(|syntax| {
            prepared_source_origin_matches(slots.resolve_prepared(owner), *syntax)
        })
    {
        return true;
    }
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let HirOrigin::Synthetic(key) = metadata.origin() else {
        return false;
    };
    let SyntheticOwner::Expr(outer) = key.owner() else {
        return false;
    };
    key.role() == SyntheticRole::DialogueContentCandidateExpression
        && key.ordinal() == 0
        && syntax_owners
            .get(&SyntheticOwner::Expr(outer))
            .is_some_and(|syntax| {
                prepared_source_origin_matches(slots.resolve_prepared(outer), *syntax)
            })
}

fn source_owner_is_prepared_at(
    owner: SyntheticOwner,
    syntax: SyntaxNodeId,
    slots: &SlotSnapshot,
) -> bool {
    match owner {
        SyntheticOwner::Expr(owner) => {
            prepared_source_origin_matches(slots.resolve_prepared(owner), syntax)
        }
        SyntheticOwner::Pattern(owner) => {
            prepared_source_origin_matches(slots.resolve_prepared(owner), syntax)
        }
        SyntheticOwner::Type(owner) => {
            prepared_source_origin_matches(slots.resolve_prepared(owner), syntax)
        }
        SyntheticOwner::Stmt(owner) => {
            prepared_source_origin_matches(slots.resolve_prepared(owner), syntax)
        }
        SyntheticOwner::Item(owner) => {
            prepared_source_origin_matches(slots.resolve_prepared(owner), syntax)
        }
        SyntheticOwner::Scope(_) | SyntheticOwner::Local(_) | SyntheticOwner::Capture(_) => false,
    }
}

fn prepared_source_origin_matches(
    metadata: Result<&crate::slot::HirSlotMetadata, crate::slot::HirSlotError>,
    syntax: SyntaxNodeId,
) -> bool {
    metadata.is_ok_and(
        |metadata| matches!(metadata.origin(), HirOrigin::Source(key) if key.syntax() == syntax),
    )
}

#[allow(
    clippy::result_large_err,
    reason = "source mismatch errors preserve both complete document identities and revisions"
)]
fn validate_component_source(
    expected: &SourceDocumentIdentity,
    actual: &SourceDocumentIdentity,
) -> Result<(), HirSourceCommitInvariantError> {
    if expected.id() != actual.id() {
        return Err(HirSourceCommitInvariantError::WrongSourceDocument {
            expected: expected.id().clone(),
            actual: actual.id().clone(),
        });
    }
    if expected.revision() != actual.revision() {
        return Err(HirSourceCommitInvariantError::StaleSourceRevision {
            expected: expected.revision(),
            actual: actual.revision(),
        });
    }
    if expected.source_len() != actual.source_len() {
        return Err(HirSourceCommitInvariantError::SourceLengthMismatch {
            expected: expected.source_len(),
            actual: actual.source_len(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_index/tests.rs"]
mod tests;
