//! Snapshot-bound typed grammar handles and syntax-owned marker kinds.

use core::marker::PhantomData;

use arcweft_source::{SourceRange, SourceSpan};

use super::{SyntaxLookupError, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId};
use crate::grammar::kinds::{AstTag, SyntaxKind};

mod sealed {
    pub trait Sealed {}
}

/// Grammar-kind predicate owned by the syntax crate.
///
/// This trait is sealed; only syntax-owned marker types can implement it.
pub trait AstKind: sealed::Sealed + Copy + 'static {
    const TAG: AstTag;

    fn accepts(kind: SyntaxKind) -> bool;

    fn exact_kind() -> Option<SyntaxKind> {
        None
    }
}

/// Sealed marker for exactly one concrete grammar kind.
pub trait ExactAstKind: AstKind {
    const KIND: SyntaxKind;
}

macro_rules! define_ast_kinds {
    ($inventory:ident, $tag:ident; $($marker:ident => $kind:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            #[doc = concat!("Typed marker for `SyntaxKind::", stringify!($kind), "`.")]
            pub struct $marker;

            impl sealed::Sealed for $marker {}

            impl AstKind for $marker {
                const TAG: AstTag = AstTag::$tag;

                fn accepts(kind: SyntaxKind) -> bool {
                    kind == SyntaxKind::$kind
                }

                fn exact_kind() -> Option<SyntaxKind> {
                    Some(SyntaxKind::$kind)
                }
            }

            impl ExactAstKind for $marker {
                const KIND: SyntaxKind = SyntaxKind::$kind;
            }
        )+

        #[cfg(test)]
        const $inventory: &[(SyntaxKind, AstTag)] = &[
            $((SyntaxKind::$kind, AstTag::$tag)),+
        ];
    };
}

macro_rules! define_fragment_root_kind {
    ($marker:ident, $tag:ident, $accepts:expr) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[doc = concat!("Typed root marker for an attached ", stringify!($tag), " fragment.")]
        pub struct $marker;

        impl sealed::Sealed for $marker {}

        impl AstKind for $marker {
            const TAG: AstTag = AstTag::$tag;

            fn accepts(kind: SyntaxKind) -> bool {
                ($accepts)(kind)
            }
        }
    };
}

define_fragment_root_kind!(
    ExpressionFragmentRootKind,
    Expression,
    SyntaxKind::is_expression
);
define_fragment_root_kind!(TypeFragmentRootKind, Type, SyntaxKind::is_type_node);
define_fragment_root_kind!(
    PatternFragmentRootKind,
    Pattern,
    SyntaxKind::is_pattern_node
);
define_fragment_root_kind!(
    StatementFragmentRootKind,
    Statement,
    SyntaxKind::is_statement
);

define_ast_kinds!(SOURCE_FILE_MARKERS, SourceFile;
    SourceFileKind => SourceFile,
);

define_ast_kinds!(ITEM_MARKERS, Item;
    ModuleDeclarationKind => ModuleDeclaration,
    UseDeclarationKind => UseDeclaration,
    FlowItemKind => FlowItem,
    FunctionItemKind => FunctionItem,
    PredicateItemKind => PredicateItem,
    ProofItemKind => ProofItem,
    TraitItemKind => TraitItem,
    ImplItemKind => ImplItem,
    EnumItemKind => EnumItem,
    StructItemKind => StructItem,
    TypeAliasItemKind => TypeAliasItem,
    ResourceDeclarationItemKind => ResourceDeclarationItem,
    CharacterDeclarationItemKind => CharacterDeclarationItem,
    ViewDeclarationItemKind => ViewDeclarationItem,
    ActionDeclarationItemKind => ActionDeclarationItem,
    ActivityDeclarationItemKind => ActivityDeclarationItem,
    SignalDeclarationItemKind => SignalDeclarationItem,
    MetricDeclarationItemKind => MetricDeclarationItem,
    LayerDeclarationItemKind => LayerDeclarationItem,
    EntryDeclarationItemKind => EntryDeclarationItem,
    ExternCapabilityItemKind => ExternCapabilityItem,
    TestItemKind => TestItem,
    BenchItemKind => BenchItem,
    SourceItemKind => SourceItem,
    StyleItemKind => StyleItem,
    ErrorItemKind => ErrorItem,
);

define_ast_kinds!(STATEMENT_MARKERS, Statement;
    AssertionStatementKind => AssertionStatement,
    LetStatementKind => LetStatement,
    AssignmentStatementKind => AssignmentStatement,
    LetElseStatementKind => LetElseStatement,
    LetChoiceStatementKind => LetChoiceStatement,
    LetScopeStatementKind => LetScopeStatement,
    LetLoopStatementKind => LetLoopStatement,
    LetActionReceiveStatementKind => LetActionReceiveStatement,
    ReturnStatementKind => ReturnStatement,
    OutStatementKind => OutStatement,
    GotoStatementKind => GotoStatement,
    DeferBlockStatementKind => DeferBlockStatement,
    DeferStatementKind => DeferStatement,
    YieldStatementKind => YieldStatement,
    SignalStatementKind => SignalStatement,
    LifetimeSetStatementKind => LifetimeSetStatement,
    WaitStatementKind => WaitStatement,
    OnStatementKind => OnStatement,
    UnsafeLifetimeStatementKind => UnsafeLifetimeStatement,
    IfStatementKind => IfStatement,
    LoopStatementKind => LoopStatement,
    WhileStatementKind => WhileStatement,
    WhileLetStatementKind => WhileLetStatement,
    ForStatementKind => ForStatement,
    MatchStatementKind => MatchStatement,
    CloseStatementKind => CloseStatement,
    SelectStatementKind => SelectStatement,
    ChoiceStatementKind => ChoiceStatement,
    SourceLocaleStatementKind => SourceLocaleStatement,
    ScopeStatementKind => ScopeStatement,
    IncludeStatementKind => IncludeStatement,
    AwaitWithStatementKind => AwaitWithStatement,
    BreakStatementKind => BreakStatement,
    ContinueStatementKind => ContinueStatement,
    ExpressionStatementKind => ExpressionStatement,
    ProofCallStatementKind => ProofCallStatement,
    ErrorStatementKind => ErrorStatement,
);

define_ast_kinds!(EXPRESSION_MARKERS, Expression;
    LiteralExpressionKind => LiteralExpression,
    EntityReferenceExpressionKind => EntityReferenceExpression,
    LifetimePathExpressionKind => LifetimePathExpression,
    PathExpressionKind => PathExpression,
    ShortVariantExpressionKind => ShortVariantExpression,
    PlaceholderExpressionKind => PlaceholderExpression,
    TupleExpressionKind => TupleExpression,
    BracketSequenceExpressionKind => BracketSequenceExpression,
    NumericBracketSequenceExpressionKind => NumericBracketSequenceExpression,
    ArrayRepeatExpressionKind => ArrayRepeatExpression,
    CallExpressionKind => CallExpression,
    SelectExpressionKind => SelectExpression,
    PostfixBracketExpressionKind => PostfixBracketExpression,
    DialogueContentApplicationExpressionKind => DialogueContentApplicationExpression,
    PipeExpressionKind => PipeExpression,
    TryExpressionKind => TryExpression,
    AwaitExpressionKind => AwaitExpression,
    ThreadExpressionKind => ThreadExpression,
    ChoiceExpressionKind => ChoiceExpression,
    RangeExpressionKind => RangeExpression,
    RecordExpressionKind => RecordExpression,
    RecordLiteralExpressionKind => RecordLiteralExpression,
    BinaryExpressionKind => BinaryExpression,
    BorrowExpressionKind => BorrowExpression,
    DereferenceExpressionKind => DereferenceExpression,
    ClosureExpressionKind => ClosureExpression,
    UnaryExpressionKind => UnaryExpression,
    BlockExpressionKind => BlockExpression,
    ComputationBlockExpressionKind => ComputationBlockExpression,
    NamedBlockExpressionKind => NamedBlockExpression,
    IfExpressionKind => IfExpression,
    IfLetExpressionKind => IfLetExpression,
    MatchExpressionKind => MatchExpression,
    MatchArmKind => MatchArm,
    CallArgumentKind => CallArgument,
    RecordFieldKind => RecordField,
    ClosureParameterKind => ClosureParameter,
    OmittedBlockTailKind => OmittedBlockTail,
    MissingExpressionKind => MissingExpression,
    ErrorExpressionKind => ErrorExpression,
);

define_ast_kinds!(PATTERN_MARKERS, Pattern;
    WildcardPatternKind => WildcardPattern,
    BindingPatternKind => BindingPattern,
    MutableBindingPatternKind => MutableBindingPattern,
    TypedBindingPatternKind => TypedBindingPattern,
    LiteralPatternKind => LiteralPattern,
    EntityReferencePatternKind => EntityReferencePattern,
    TuplePatternKind => TuplePattern,
    RecordPatternKind => RecordPattern,
    RecordPatternFieldKind => RecordPatternField,
    VariantPatternKind => VariantPattern,
    SequencePatternKind => SequencePattern,
    RestPatternKind => RestPattern,
    WholeBindingPatternKind => WholeBindingPattern,
    OrPatternKind => OrPattern,
    MissingPatternKind => MissingPattern,
    ErrorPatternKind => ErrorPattern,
);

define_ast_kinds!(TYPE_MARKERS, Type;
    PrimitiveTypeKind => PrimitiveType,
    PathTypeKind => PathType,
    GenericApplicationTypeKind => GenericApplicationType,
    TupleTypeKind => TupleType,
    ReferenceTypeKind => ReferenceType,
    SliceTypeKind => SliceType,
    FunctionTypeKind => FunctionType,
    SumTypeKind => SumType,
    TypeArgumentKind => TypeArgument,
    MissingTypeKind => MissingType,
    ErrorTypeKind => ErrorType,
);

define_ast_kinds!(ATTRIBUTE_MARKERS, Attribute;
    InnerAttributeKind => InnerAttribute,
    OuterAttributeKind => OuterAttribute,
    DocBlockKind => DocBlock,
);

define_ast_kinds!(NAME_MARKERS, Name;
    NameDefinitionKind => NameDefinition,
    NameReferenceKind => NameReference,
    MissingNameKind => MissingName,
);

define_ast_kinds!(PATH_MARKERS, Path;
    PathKind => Path,
);

define_ast_kinds!(BODY_MARKERS, Body;
    ExpressionBodyKind => ExpressionBody,
    PredicateBodyKind => PredicateBody,
    ProofBodyKind => ProofBody,
    FunctionBodyKind => FunctionBody,
    FlowBodyKind => FlowBody,
    ResourceBodyKind => ResourceBody,
    CharacterBodyKind => CharacterBody,
    ViewDeclarationBodyKind => ViewDeclarationBody,
    ActivityBodyKind => ActivityBody,
    MetricBodyKind => MetricBody,
    LayerBodyKind => LayerBody,
    StyleBodyKind => StyleBody,
    EntryBodyKind => EntryBody,
    BlockKind => Block,
    ChoiceBodyKind => ChoiceBody,
    ChoiceOptionBodyKind => ChoiceOptionBody,
    ChoiceViewBodyKind => ChoiceViewBody,
    ChoicePlanBodyKind => ChoicePlanBody,
    PredicateBlockKind => PredicateBlock,
    ProofBlockKind => ProofBlock,
);

define_ast_kinds!(DELIMITER_MARKERS, Delimiter;
    OpenBraceKind => OpenBraceNode,
    CloseBraceKind => CloseBraceNode,
    OpenParenKind => OpenParenNode,
    CloseParenKind => CloseParenNode,
    OpenBracketKind => OpenBracketNode,
    CloseBracketKind => CloseBracketNode,
    OpenAngleKind => OpenAngleNode,
    CloseAngleKind => CloseAngleNode,
    EqualsKind => EqualsNode,
    ColonKind => ColonNode,
    ThinArrowKind => ThinArrowNode,
    ForInKind => ForInNode,
    RestParameterMarkerKind => RestParameterMarker,
);

define_ast_kinds!(RICH_TEXT_MARKERS, RichText;
    RichTextTagKind => RichTextTag,
    RichTextEndTagKind => RichTextEndTag,
    RichTextTagNameKind => RichTextTagName,
    RichTextArgumentPayloadKind => RichTextArgumentPayload,
    RichTextFxCallPayloadKind => RichTextFxCallPayload,
    RichTextDialogueCallPayloadKind => RichTextDialogueCallPayload,
    RichTextConditionPayloadKind => RichTextConditionPayload,
    RichTextPositionalArgumentKind => RichTextPositionalArgument,
    RichTextNamedArgumentKind => RichTextNamedArgument,
    RichTextInvalidArgumentKind => RichTextInvalidArgument,
    RichTextArgumentKeyKind => RichTextArgumentKey,
    RichTextArgumentEqualsKind => RichTextArgumentEquals,
    RichTextArgumentValueKind => RichTextArgumentValue,
    RichTextArgumentTokenKind => RichTextArgumentToken,
    RichTextArgumentContentKind => RichTextArgumentContent,
    RichTextArgumentQuoteKind => RichTextArgumentQuote,
    RichTextMissingArgumentValueKind => RichTextMissingArgumentValue,
    RichTextInvalidArgumentIssueKind => RichTextInvalidArgumentIssue,
);

define_ast_kinds!(RECOVERY_MARKERS, Recovery;
    WrongFamilyReferenceKind => WrongFamilyReference,
    MissingDeclarationIdKind => MissingDeclarationId,
    MissingMemberValueKind => MissingMemberValue,
    ErrorDeclarationMemberKind => ErrorDeclarationMember,
    MissingBodyKind => MissingBody,
    MissingTokenNodeKind => MissingTokenNode,
    ErrorNodeKind => ErrorNode,
);

define_ast_kinds!(DECLARATION_PART_MARKERS, DeclarationPart;
    VisibilityKind => Visibility,
    DeclarationHeaderKind => DeclarationHeader,
    DeclarationPublicIdKind => DeclarationPublicId,
    SurfaceAliasKind => SurfaceAlias,
    PostfixBracketPayloadKind => PostfixBracketPayload,
    DialogueContentKind => DialogueContent,
    DialogueTextKind => DialogueText,
    DialogueRawKind => DialogueRaw,
    DialogueEscapeKind => DialogueEscape,
    DialogueRubyKind => DialogueRuby,
    DialogueInterpolationKind => DialogueInterpolation,
    DialogueControlKind => DialogueControl,
    DialogueMarkKind => DialogueMark,
    DialogueLineBreakKind => DialogueLineBreak,
    DialogueErrorKind => DialogueError,
    GenericParameterGroupKind => GenericParameterGroup,
    GenericParameterKind => GenericParameter,
    LifetimeParameterKind => LifetimeParameter,
    TypeParameterKind => TypeParameter,
    FixedParameterGroupKind => FixedParameterGroup,
    ParameterKind => Parameter,
    WhereClauseKind => WhereClause,
    WherePredicateKind => WherePredicate,
    ReturnTypeKind => ReturnType,
    RequiresClauseKind => RequiresClause,
    EnsuresClauseKind => EnsuresClause,
    InvariantClauseKind => InvariantClause,
    AssumeClauseKind => AssumeClause,
    ReadsClauseKind => ReadsClause,
    EffectsClauseKind => EffectsClause,
    NoEffectClauseKind => NoEffectClause,
    ModifiesClauseKind => ModifiesClause,
    DecreasesClauseKind => DecreasesClause,
    SelectBranchKind => SelectBranch,
    AwaitWithBranchKind => AwaitWithBranch,
    ChoiceIfItemKind => ChoiceIfItem,
    ChoiceIfBranchKind => ChoiceIfBranch,
    ChoiceForItemKind => ChoiceForItem,
    ChoiceMatchItemKind => ChoiceMatchItem,
    ChoiceMatchArmKind => ChoiceMatchArm,
    ChoiceOptionKind => ChoiceOption,
    ChoiceOptionForKind => ChoiceOptionFor,
    ChoiceLabelFieldKind => ChoiceLabelField,
    ChoiceIdFieldKind => ChoiceIdField,
    ChoiceValueFieldKind => ChoiceValueField,
    ChoiceVisibleFieldKind => ChoiceVisibleField,
    ChoiceEnabledFieldKind => ChoiceEnabledField,
    ChoiceOrderFieldKind => ChoiceOrderField,
    ChoiceHotkeyFieldKind => ChoiceHotkeyField,
    ChoiceViewFieldKind => ChoiceViewField,
    ChoiceSelectFieldKind => ChoiceSelectField,
    ChoiceCompactArmKind => ChoiceCompactArm,
    ChoiceGotoActionKind => ChoiceGotoAction,
    ChoiceOutActionKind => ChoiceOutAction,
    ChoicePlanKind => ChoicePlan,
    ChoicePlanAssignmentKind => ChoicePlanAssignment,
    ChoicePlanTimeoutKind => ChoicePlanTimeout,
    ChoicePlanCancelKind => ChoicePlanCancel,
    ChoicePlanOnSelectKind => ChoicePlanOnSelect,
    InputTriggerPatternKind => InputTriggerPattern,
    EventTriggerPatternKind => EventTriggerPattern,
    SignalTriggerPatternKind => SignalTriggerPattern,
    TimeoutTriggerPatternKind => TimeoutTriggerPattern,
    MarkTriggerPatternKind => MarkTriggerPattern,
    SelectTriggerPatternKind => SelectTriggerPattern,
    TaskTriggerPatternKind => TaskTriggerPattern,
    ScopeTriggerPatternKind => ScopeTriggerPattern,
    ResourceFieldInitializerKind => ResourceFieldInitializer,
    CharacterDisplayNameMemberKind => CharacterDisplayNameMember,
    ViewExportBlockKind => ViewExportBlock,
    ViewExportDeclarationKind => ViewExportDeclaration,
    ViewFragmentKind => ViewFragment,
    ActionSignatureKind => ActionSignature,
    ActivityModeMemberKind => ActivityModeMember,
    ActivityLifecycleMemberKind => ActivityLifecycleMember,
    ActivityInputBlockKind => ActivityInputBlock,
    ActivityOutputBlockKind => ActivityOutputBlock,
    ActivityPortKind => ActivityPort,
    ActivityContractBlockKind => ActivityContractBlock,
    SignalObservableTypeKind => SignalObservableType,
    MetricKindKind => MetricKind,
    MetricUnitMemberKind => MetricUnitMember,
    MetricLabelsBlockKind => MetricLabelsBlock,
    MetricLabelKind => MetricLabel,
    MetricBucketsMemberKind => MetricBucketsMember,
    LayerKindNodeKind => LayerKindNode,
    LayerMemberKind => LayerMember,
    LayerPolicyValueKind => LayerPolicyValue,
    RetainedReferenceKind => RetainedReference,
    StyleTokenDeclarationKind => StyleTokenDeclaration,
    StyleRuleKind => StyleRule,
    StyleSelectorKind => StyleSelector,
    StyleSelectorSequenceKind => StyleSelectorSequence,
    StylePropertyDeclarationKind => StylePropertyDeclaration,
    StyleEnvironmentBlockKind => StyleEnvironmentBlock,
    StyleEnvironmentConditionKind => StyleEnvironmentCondition,
    StyleEnvironmentClauseKind => StyleEnvironmentClause,
    EntryRoleBindingKind => EntryRoleBinding,
    EntryGotoKind => EntryGoto,
    EntryRouteKind => EntryRoute,
    EntryRouteBindingKind => EntryRouteBinding,
    EntryOptionKind => EntryOption,
);

/// Typed handle that cannot detach from its immutable grammar snapshot.
pub struct AstNode<K: AstKind> {
    syntax: SyntaxNodeHandle,
    marker: PhantomData<fn() -> K>,
}

impl<K: AstKind> Clone for AstNode<K> {
    fn clone(&self) -> Self {
        Self {
            syntax: self.syntax.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: AstKind> core::fmt::Debug for AstNode<K> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AstNode")
            .field("kind", &self.syntax.kind())
            .field("tag", &K::TAG)
            .field("id", &self.id())
            .field("snapshot", self.snapshot_id())
            .finish()
    }
}

impl<K: AstKind> PartialEq for AstNode<K> {
    fn eq(&self, other: &Self) -> bool {
        self.syntax == other.syntax
    }
}

impl<K: AstKind> Eq for AstNode<K> {}

impl<K: AstKind> AstNode<K> {
    pub(super) fn new(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxLookupError> {
        if syntax.tag() != K::TAG {
            return Err(SyntaxLookupError::AstTagMismatch {
                id: syntax.id(),
                expected: K::TAG,
                actual: syntax.tag(),
            });
        }
        if !K::accepts(syntax.kind()) {
            return Err(match K::exact_kind() {
                Some(expected) => SyntaxLookupError::KindMismatch {
                    id: syntax.id(),
                    expected,
                    actual: syntax.kind(),
                },
                None => SyntaxLookupError::KindPredicateMismatch {
                    id: syntax.id(),
                    expected: K::TAG,
                    actual: syntax.kind(),
                },
            });
        }
        Ok(Self {
            syntax,
            marker: PhantomData,
        })
    }

    pub fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub fn kind(&self) -> SyntaxKind {
        self.syntax.kind()
    }

    pub fn role(&self) -> crate::grammar::kinds::SyntaxRole {
        self.syntax.role()
    }

    pub fn range(&self) -> SourceRange {
        self.syntax.range()
    }

    /// Exact revision-bound source span occupied by this typed node.
    pub fn source_span(&self) -> SourceSpan {
        self.syntax.source_span()
    }

    /// Exact UTF-8 source slice retained with this typed node.
    pub fn source_text(&self) -> &str {
        self.syntax.source_text()
    }

    pub fn is_same_reconciled_node(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ATTRIBUTE_MARKERS, AstKind, BODY_MARKERS, DECLARATION_PART_MARKERS, DELIMITER_MARKERS,
        EXPRESSION_MARKERS, ExpressionFragmentRootKind, ITEM_MARKERS, NAME_MARKERS, PATH_MARKERS,
        PATTERN_MARKERS, PatternFragmentRootKind, RECOVERY_MARKERS, RICH_TEXT_MARKERS,
        SOURCE_FILE_MARKERS, STATEMENT_MARKERS, StatementFragmentRootKind, TYPE_MARKERS,
        TypeFragmentRootKind,
    };
    use crate::grammar::kinds::{AstTag, IdentityClass, SyntaxKind};

    #[test]
    fn every_identity_bearing_kind_has_one_exact_marker_contract() {
        let groups: &[&[(SyntaxKind, AstTag)]] = &[
            SOURCE_FILE_MARKERS,
            ITEM_MARKERS,
            STATEMENT_MARKERS,
            EXPRESSION_MARKERS,
            PATTERN_MARKERS,
            TYPE_MARKERS,
            ATTRIBUTE_MARKERS,
            NAME_MARKERS,
            PATH_MARKERS,
            BODY_MARKERS,
            DELIMITER_MARKERS,
            RICH_TEXT_MARKERS,
            RECOVERY_MARKERS,
            DECLARATION_PART_MARKERS,
        ];
        let mut inventory = BTreeMap::new();
        for &(kind, tag) in groups.iter().flat_map(|group| group.iter()) {
            assert_eq!(
                inventory.insert(kind, tag),
                None,
                "duplicate exact marker for {kind:?}"
            );
        }

        for &kind in SyntaxKind::ALL {
            match kind.identity_class() {
                IdentityClass::IdentityBearing => assert_eq!(
                    inventory.get(&kind).copied(),
                    kind.ast_tag(),
                    "exact marker contract for {kind:?}"
                ),
                IdentityClass::StructuralWrapper | IdentityClass::Token => {
                    assert_eq!(inventory.get(&kind), None, "non-node marker for {kind:?}");
                }
            }
        }
    }

    #[test]
    fn fragment_root_markers_accept_only_their_typed_node_family() {
        assert!(ExpressionFragmentRootKind::accepts(
            SyntaxKind::BinaryExpression
        ));
        assert!(ExpressionFragmentRootKind::accepts(
            SyntaxKind::MissingExpression
        ));
        assert!(!ExpressionFragmentRootKind::accepts(SyntaxKind::PathType));

        assert!(TypeFragmentRootKind::accepts(
            SyntaxKind::GenericApplicationType
        ));
        assert!(!TypeFragmentRootKind::accepts(SyntaxKind::BindingPattern));

        assert!(PatternFragmentRootKind::accepts(SyntaxKind::VariantPattern));
        assert!(!PatternFragmentRootKind::accepts(SyntaxKind::LetStatement));

        assert!(StatementFragmentRootKind::accepts(SyntaxKind::LetStatement));
        assert!(!StatementFragmentRootKind::accepts(
            SyntaxKind::CallExpression
        ));
    }
}
