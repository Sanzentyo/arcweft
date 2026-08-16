//! Final grammar-node and token vocabulary for the lossless attached parser.
//!
//! The identity and typed-family classifiers intentionally enumerate every
//! grammar kind in one exhaustive match. Their size is the audit surface that
//! prevents a new kind from silently inheriting identity or attachment policy.

pub(crate) use super::roles::{
    ActivityPolicySyntaxValue, LayerKindSyntaxValue, LayerMemberSyntaxKind, LayerPolicySyntaxValue,
    MetricKindSyntaxValue, SyntaxRole, SyntaxRoleClass,
};

macro_rules! define_syntax_kinds {
    ($($kind:ident),+ $(,)?) => {
        /// Grammar node and token kinds produced by the final event parser.
        ///
        /// Raw Rowan conversion remains crate-private.
        /// Tokens never receive syntax identity; structural wrappers retain
        /// layout without becoming a second semantic-parent authority.
        #[repr(u16)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum SyntaxKind {
            $($kind),+
        }

        impl SyntaxKind {
            /// Exhaustive grammar vocabulary in discriminant order.
            #[cfg(test)]
            pub(crate) const ALL: &'static [Self] = &[$(Self::$kind),+];
        }
    };
}

define_syntax_kinds! {
    SourceFile,
    ItemList,
    StatementList,
    ExpressionList,
    ParameterList,
    GenericParameterList,
    WherePredicateList,
    AttributeList,
    FieldList,
    ArgumentList,
    RichTextArgumentList,
    MatchArmList,
    LogicalLine,
    IndentedSuite,
    FenceBody,
    DelimitedGroup,
    ModuleDeclaration,
    UseDeclaration,
    FlowItem,
    FunctionItem,
    PredicateItem,
    ProofItem,
    TraitItem,
    ImplItem,
    EnumItem,
    StructItem,
    TypeAliasItem,
    ResourceDeclarationItem,
    CharacterDeclarationItem,
    ViewDeclarationItem,
    ActionDeclarationItem,
    ActivityDeclarationItem,
    SignalDeclarationItem,
    MetricDeclarationItem,
    LayerDeclarationItem,
    EntryDeclarationItem,
    ExternCapabilityItem,
    TestItem,
    BenchItem,
    StyleItem,
    ErrorItem,
    InnerAttribute,
    OuterAttribute,
    DocBlock,
    Visibility,
    DeclarationHeader,
    DeclarationPublicId,
    SurfaceAlias,
    NameDefinition,
    NameReference,
    Path,
    PathSegment,
    GenericParameterGroup,
    GenericParameter,
    LifetimeParameter,
    TypeParameter,
    FixedParameterGroup,
    Parameter,
    RestParameterMarker,
    WhereClause,
    WherePredicate,
    ReturnType,
    RequiresClause,
    EnsuresClause,
    InvariantClause,
    AssumeClause,
    ReadsClause,
    EffectsClause,
    NoEffectClause,
    ModifiesClause,
    DecreasesClause,
    ExpressionBody,
    PredicateBody,
    ProofBody,
    FunctionBody,
    FlowBody,
    ResourceBody,
    ResourceFieldInitializer,
    CharacterBody,
    CharacterDisplayNameMember,
    ViewDeclarationBody,
    ViewExportBlock,
    ViewExportDeclaration,
    ViewFragment,
    ActionSignature,
    ActivityBody,
    ActivityModeMember,
    ActivityLifecycleMember,
    ActivityInputBlock,
    ActivityOutputBlock,
    ActivityPort,
    ActivityContractBlock,
    SignalObservableType,
    MetricKind,
    MetricBody,
    MetricUnitMember,
    MetricLabelsBlock,
    MetricLabel,
    MetricBucketsMember,
    LayerKindNode,
    LayerBody,
    LayerMember,
    LayerPolicyValue,
    RetainedReference,
    WrongFamilyReference,
    MissingDeclarationId,
    MissingMemberValue,
    ErrorDeclarationMember,
    StyleBody,
    StyleTokenDeclaration,
    StyleRule,
    StyleSelector,
    StyleSelectorSequence,
    StylePropertyDeclaration,
    StyleEnvironmentBlock,
    StyleEnvironmentCondition,
    StyleEnvironmentClause,
    EntryBody,
    EntryRoleBinding,
    EntryGoto,
    EntryRoute,
    EntryRouteBinding,
    EntryOption,
    Block,
    ChoiceBody,
    ChoiceOptionBody,
    ChoiceViewBody,
    ChoicePlanBody,
    PredicateBlock,
    ProofBlock,
    OpenBraceNode,
    CloseBraceNode,
    OpenParenNode,
    CloseParenNode,
    OpenBracketNode,
    CloseBracketNode,
    OpenAngleNode,
    CloseAngleNode,
    EqualsNode,
    ColonNode,
    ThinArrowNode,
    ForInNode,
    AssertionStatement,
    LetStatement,
    AssignmentStatement,
    LetElseStatement,
    LetChoiceStatement,
    LetScopeStatement,
    LetLoopStatement,
    LetActionReceiveStatement,
    ReturnStatement,
    OutStatement,
    GotoStatement,
    DeferBlockStatement,
    DeferStatement,
    YieldStatement,
    SignalStatement,
    LifetimeSetStatement,
    WaitStatement,
    OnStatement,
    UnsafeLifetimeStatement,
    IfStatement,
    LoopStatement,
    WhileStatement,
    WhileLetStatement,
    ForStatement,
    MatchStatement,
    CloseStatement,
    SelectStatement,
    ChoiceStatement,
    ChoiceIfItem,
    ChoiceIfBranch,
    ChoiceForItem,
    ChoiceMatchItem,
    ChoiceMatchArm,
    ChoiceOption,
    ChoiceOptionFor,
    ChoiceLabelField,
    ChoiceIdField,
    ChoiceValueField,
    ChoiceVisibleField,
    ChoiceEnabledField,
    ChoiceOrderField,
    ChoiceHotkeyField,
    ChoiceViewField,
    ChoiceSelectField,
    ChoiceCompactArm,
    ChoiceGotoAction,
    ChoiceOutAction,
    ChoicePlan,
    ChoicePlanAssignment,
    ChoicePlanTimeout,
    ChoicePlanCancel,
    ChoicePlanOnSelect,
    InputTriggerPattern,
    EventTriggerPattern,
    SignalTriggerPattern,
    TimeoutTriggerPattern,
    MarkTriggerPattern,
    SelectTriggerPattern,
    TaskTriggerPattern,
    ScopeTriggerPattern,
    SourceLocaleStatement,
    ScopeStatement,
    IncludeStatement,
    SelectBranch,
    AwaitWithBranch,
    BreakStatement,
    ContinueStatement,
    ExpressionStatement,
    ProofCallStatement,
    ErrorStatement,
    LiteralExpression,
    EntityReferenceExpression,
    LifetimePathExpression,
    PathExpression,
    ShortVariantExpression,
    PlaceholderExpression,
    TupleExpression,
    BracketSequenceExpression,
    NumericBracketSequenceExpression,
    ArrayRepeatExpression,
    CallExpression,
    SelectExpression,
    PostfixBracketExpression,
    DialogueContentApplicationExpression,
    PostfixBracketPayload,
    DialogueContent,
    DialogueText,
    DialogueRaw,
    DialogueEscape,
    DialogueRuby,
    DialogueInterpolation,
    DialogueControl,
    DialogueMark,
    DialogueLineBreak,
    DialogueError,
    RichTextTag,
    RichTextEndTag,
    RichTextTagName,
    RichTextArgumentPayload,
    RichTextFxCallPayload,
    RichTextDialogueCallPayload,
    RichTextConditionPayload,
    RichTextPositionalArgument,
    RichTextNamedArgument,
    RichTextInvalidArgument,
    RichTextArgumentKey,
    RichTextArgumentEquals,
    RichTextArgumentValue,
    RichTextArgumentToken,
    RichTextArgumentContent,
    RichTextArgumentQuote,
    RichTextMissingArgumentValue,
    RichTextInvalidArgumentIssue,
    PipeExpression,
    TryExpression,
    AwaitExpression,
    ThreadExpression,
    ChoiceExpression,
    RangeExpression,
    RecordExpression,
    RecordLiteralExpression,
    BinaryExpression,
    BorrowExpression,
    DereferenceExpression,
    ClosureExpression,
    UnaryExpression,
    BlockExpression,
    ComputationBlockExpression,
    NamedBlockExpression,
    IfExpression,
    IfLetExpression,
    MatchExpression,
    MatchArm,
    CallArgument,
    RecordField,
    ClosureParameter,
    OmittedBlockTail,
    MissingExpression,
    ErrorExpression,
    WildcardPattern,
    BindingPattern,
    MutableBindingPattern,
    TypedBindingPattern,
    LiteralPattern,
    EntityReferencePattern,
    TuplePattern,
    RecordPattern,
    RecordPatternField,
    VariantPattern,
    SequencePattern,
    RestPattern,
    WholeBindingPattern,
    OrPattern,
    MissingPattern,
    ErrorPattern,
    PrimitiveType,
    PathType,
    GenericApplicationType,
    TupleType,
    ReferenceType,
    SliceType,
    FunctionType,
    SumType,
    TypeArgument,
    MissingType,
    ErrorType,
    MissingName,
    MissingBody,
    MissingTokenNode,
    ErrorNode,
    WhitespaceToken,
    NewlineToken,
    CommentToken,
    DocCommentToken,
    IdentifierToken,
    LifetimeToken,
    NumberToken,
    StringToken,
    UnterminatedStringToken,
    RawStringToken,
    CharacterToken,
    EntityReferenceToken,
    KeywordToken,
    PunctuationToken,
    TextToken,
    ErrorToken,
    MissingToken,
    EofToken,
}

/// Whether a grammar kind owns session identity or only lossless structure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IdentityClass {
    IdentityBearing,
    StructuralWrapper,
    Token,
}

/// Typed attachment family owned by one identity-bearing grammar node.
///
/// This is deliberately coarser than [`SyntaxKind`]. Exact marker casts still
/// validate the concrete kind, while family tags let the attachment layer
/// build item/expression/statement/pattern/type inventories without retaining
/// or reparsing the detached surface AST.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AstTag {
    SourceFile,
    Item,
    Statement,
    Expression,
    Pattern,
    Type,
    Attribute,
    Name,
    Path,
    DeclarationPart,
    Body,
    Delimiter,
    RichText,
    Recovery,
}

impl SyntaxKind {
    /// Returns whether this kind is a token rather than a Rowan node.
    pub const fn is_token(self) -> bool {
        matches!(self.identity_class(), IdentityClass::Token)
    }

    pub(crate) const fn token_display_name(self) -> Option<&'static str> {
        match self {
            Self::WhitespaceToken => Some("whitespace"),
            Self::NewlineToken => Some("newline"),
            Self::CommentToken => Some("comment"),
            Self::DocCommentToken => Some("documentation comment"),
            Self::IdentifierToken => Some("identifier"),
            Self::LifetimeToken => Some("lifetime"),
            Self::NumberToken => Some("number"),
            Self::StringToken => Some("string"),
            Self::UnterminatedStringToken => Some("string terminator"),
            Self::RawStringToken => Some("raw string"),
            Self::CharacterToken => Some("character"),
            Self::EntityReferenceToken => Some("entity reference"),
            Self::KeywordToken => Some("keyword"),
            Self::PunctuationToken => Some("punctuation"),
            Self::TextToken => Some("text"),
            Self::ErrorToken => Some("valid token"),
            Self::MissingToken => Some("missing token"),
            Self::EofToken => Some("end of input"),
            _ => None,
        }
    }

    /// Returns the identity policy owned by this grammar kind.
    #[allow(
        clippy::too_many_lines,
        reason = "the wildcard-free identity table must enumerate every grammar kind explicitly"
    )]
    pub const fn identity_class(self) -> IdentityClass {
        match self {
            Self::ItemList
            | Self::StatementList
            | Self::ExpressionList
            | Self::ParameterList
            | Self::GenericParameterList
            | Self::WherePredicateList
            | Self::AttributeList
            | Self::FieldList
            | Self::ArgumentList
            | Self::RichTextArgumentList
            | Self::MatchArmList
            | Self::LogicalLine
            | Self::IndentedSuite
            | Self::FenceBody
            | Self::DelimitedGroup
            | Self::PathSegment => IdentityClass::StructuralWrapper,
            Self::WhitespaceToken
            | Self::NewlineToken
            | Self::CommentToken
            | Self::DocCommentToken
            | Self::IdentifierToken
            | Self::LifetimeToken
            | Self::NumberToken
            | Self::StringToken
            | Self::UnterminatedStringToken
            | Self::RawStringToken
            | Self::CharacterToken
            | Self::EntityReferenceToken
            | Self::KeywordToken
            | Self::PunctuationToken
            | Self::TextToken
            | Self::ErrorToken
            | Self::MissingToken
            | Self::EofToken => IdentityClass::Token,
            Self::SourceFile
            | Self::ModuleDeclaration
            | Self::UseDeclaration
            | Self::FlowItem
            | Self::FunctionItem
            | Self::PredicateItem
            | Self::ProofItem
            | Self::TraitItem
            | Self::ImplItem
            | Self::EnumItem
            | Self::StructItem
            | Self::TypeAliasItem
            | Self::ResourceDeclarationItem
            | Self::CharacterDeclarationItem
            | Self::ViewDeclarationItem
            | Self::ActionDeclarationItem
            | Self::ActivityDeclarationItem
            | Self::SignalDeclarationItem
            | Self::MetricDeclarationItem
            | Self::LayerDeclarationItem
            | Self::EntryDeclarationItem
            | Self::ExternCapabilityItem
            | Self::TestItem
            | Self::BenchItem
            | Self::StyleItem
            | Self::ErrorItem
            | Self::InnerAttribute
            | Self::OuterAttribute
            | Self::DocBlock
            | Self::Visibility
            | Self::DeclarationHeader
            | Self::DeclarationPublicId
            | Self::SurfaceAlias
            | Self::NameDefinition
            | Self::NameReference
            | Self::Path
            | Self::GenericParameterGroup
            | Self::GenericParameter
            | Self::LifetimeParameter
            | Self::TypeParameter
            | Self::FixedParameterGroup
            | Self::Parameter
            | Self::RestParameterMarker
            | Self::WhereClause
            | Self::WherePredicate
            | Self::ReturnType
            | Self::RequiresClause
            | Self::EnsuresClause
            | Self::InvariantClause
            | Self::AssumeClause
            | Self::ReadsClause
            | Self::EffectsClause
            | Self::NoEffectClause
            | Self::ModifiesClause
            | Self::DecreasesClause
            | Self::ExpressionBody
            | Self::PredicateBody
            | Self::ProofBody
            | Self::FunctionBody
            | Self::FlowBody
            | Self::ResourceBody
            | Self::ResourceFieldInitializer
            | Self::CharacterBody
            | Self::CharacterDisplayNameMember
            | Self::ViewDeclarationBody
            | Self::ViewExportBlock
            | Self::ViewExportDeclaration
            | Self::ViewFragment
            | Self::ActionSignature
            | Self::ActivityBody
            | Self::ActivityModeMember
            | Self::ActivityLifecycleMember
            | Self::ActivityInputBlock
            | Self::ActivityOutputBlock
            | Self::ActivityPort
            | Self::ActivityContractBlock
            | Self::SignalObservableType
            | Self::MetricKind
            | Self::MetricBody
            | Self::MetricUnitMember
            | Self::MetricLabelsBlock
            | Self::MetricLabel
            | Self::MetricBucketsMember
            | Self::LayerKindNode
            | Self::LayerBody
            | Self::LayerMember
            | Self::LayerPolicyValue
            | Self::RetainedReference
            | Self::WrongFamilyReference
            | Self::MissingDeclarationId
            | Self::MissingMemberValue
            | Self::ErrorDeclarationMember
            | Self::StyleBody
            | Self::StyleTokenDeclaration
            | Self::StyleRule
            | Self::StyleSelector
            | Self::StyleSelectorSequence
            | Self::StylePropertyDeclaration
            | Self::StyleEnvironmentBlock
            | Self::StyleEnvironmentCondition
            | Self::StyleEnvironmentClause
            | Self::EntryBody
            | Self::EntryRoleBinding
            | Self::EntryGoto
            | Self::EntryRoute
            | Self::EntryRouteBinding
            | Self::EntryOption
            | Self::Block
            | Self::ChoiceBody
            | Self::ChoiceOptionBody
            | Self::ChoiceViewBody
            | Self::ChoicePlanBody
            | Self::PredicateBlock
            | Self::ProofBlock
            | Self::OpenBraceNode
            | Self::CloseBraceNode
            | Self::OpenParenNode
            | Self::CloseParenNode
            | Self::OpenBracketNode
            | Self::CloseBracketNode
            | Self::OpenAngleNode
            | Self::CloseAngleNode
            | Self::EqualsNode
            | Self::ColonNode
            | Self::ThinArrowNode
            | Self::ForInNode
            | Self::AssertionStatement
            | Self::LetStatement
            | Self::AssignmentStatement
            | Self::LetElseStatement
            | Self::LetChoiceStatement
            | Self::LetScopeStatement
            | Self::LetLoopStatement
            | Self::LetActionReceiveStatement
            | Self::ReturnStatement
            | Self::OutStatement
            | Self::GotoStatement
            | Self::DeferBlockStatement
            | Self::DeferStatement
            | Self::YieldStatement
            | Self::SignalStatement
            | Self::LifetimeSetStatement
            | Self::WaitStatement
            | Self::OnStatement
            | Self::UnsafeLifetimeStatement
            | Self::IfStatement
            | Self::LoopStatement
            | Self::WhileStatement
            | Self::WhileLetStatement
            | Self::ForStatement
            | Self::MatchStatement
            | Self::CloseStatement
            | Self::SelectStatement
            | Self::ChoiceStatement
            | Self::ChoiceIfItem
            | Self::ChoiceIfBranch
            | Self::ChoiceForItem
            | Self::ChoiceMatchItem
            | Self::ChoiceMatchArm
            | Self::ChoiceOption
            | Self::ChoiceOptionFor
            | Self::ChoiceLabelField
            | Self::ChoiceIdField
            | Self::ChoiceValueField
            | Self::ChoiceVisibleField
            | Self::ChoiceEnabledField
            | Self::ChoiceOrderField
            | Self::ChoiceHotkeyField
            | Self::ChoiceViewField
            | Self::ChoiceSelectField
            | Self::ChoiceCompactArm
            | Self::ChoiceGotoAction
            | Self::ChoiceOutAction
            | Self::ChoicePlan
            | Self::ChoicePlanAssignment
            | Self::ChoicePlanTimeout
            | Self::ChoicePlanCancel
            | Self::ChoicePlanOnSelect
            | Self::InputTriggerPattern
            | Self::EventTriggerPattern
            | Self::SignalTriggerPattern
            | Self::TimeoutTriggerPattern
            | Self::MarkTriggerPattern
            | Self::SelectTriggerPattern
            | Self::TaskTriggerPattern
            | Self::ScopeTriggerPattern
            | Self::SourceLocaleStatement
            | Self::ScopeStatement
            | Self::IncludeStatement
            | Self::SelectBranch
            | Self::AwaitWithBranch
            | Self::BreakStatement
            | Self::ContinueStatement
            | Self::ExpressionStatement
            | Self::ProofCallStatement
            | Self::ErrorStatement
            | Self::LiteralExpression
            | Self::EntityReferenceExpression
            | Self::LifetimePathExpression
            | Self::PathExpression
            | Self::ShortVariantExpression
            | Self::PlaceholderExpression
            | Self::TupleExpression
            | Self::BracketSequenceExpression
            | Self::NumericBracketSequenceExpression
            | Self::ArrayRepeatExpression
            | Self::CallExpression
            | Self::SelectExpression
            | Self::PostfixBracketExpression
            | Self::DialogueContentApplicationExpression
            | Self::PostfixBracketPayload
            | Self::DialogueContent
            | Self::DialogueText
            | Self::DialogueRaw
            | Self::DialogueEscape
            | Self::DialogueRuby
            | Self::DialogueInterpolation
            | Self::DialogueControl
            | Self::DialogueMark
            | Self::DialogueLineBreak
            | Self::DialogueError
            | Self::RichTextTag
            | Self::RichTextEndTag
            | Self::RichTextTagName
            | Self::RichTextArgumentPayload
            | Self::RichTextFxCallPayload
            | Self::RichTextDialogueCallPayload
            | Self::RichTextConditionPayload
            | Self::RichTextPositionalArgument
            | Self::RichTextNamedArgument
            | Self::RichTextInvalidArgument
            | Self::RichTextArgumentKey
            | Self::RichTextArgumentEquals
            | Self::RichTextArgumentValue
            | Self::RichTextArgumentToken
            | Self::RichTextArgumentContent
            | Self::RichTextArgumentQuote
            | Self::RichTextMissingArgumentValue
            | Self::RichTextInvalidArgumentIssue
            | Self::PipeExpression
            | Self::TryExpression
            | Self::AwaitExpression
            | Self::ThreadExpression
            | Self::ChoiceExpression
            | Self::RangeExpression
            | Self::RecordExpression
            | Self::RecordLiteralExpression
            | Self::BinaryExpression
            | Self::BorrowExpression
            | Self::DereferenceExpression
            | Self::ClosureExpression
            | Self::UnaryExpression
            | Self::BlockExpression
            | Self::ComputationBlockExpression
            | Self::NamedBlockExpression
            | Self::IfExpression
            | Self::IfLetExpression
            | Self::MatchExpression
            | Self::MatchArm
            | Self::CallArgument
            | Self::RecordField
            | Self::ClosureParameter
            | Self::OmittedBlockTail
            | Self::MissingExpression
            | Self::ErrorExpression
            | Self::WildcardPattern
            | Self::BindingPattern
            | Self::MutableBindingPattern
            | Self::TypedBindingPattern
            | Self::LiteralPattern
            | Self::EntityReferencePattern
            | Self::TuplePattern
            | Self::RecordPattern
            | Self::RecordPatternField
            | Self::VariantPattern
            | Self::SequencePattern
            | Self::RestPattern
            | Self::WholeBindingPattern
            | Self::OrPattern
            | Self::MissingPattern
            | Self::ErrorPattern
            | Self::PrimitiveType
            | Self::PathType
            | Self::GenericApplicationType
            | Self::TupleType
            | Self::ReferenceType
            | Self::SliceType
            | Self::FunctionType
            | Self::SumType
            | Self::TypeArgument
            | Self::MissingType
            | Self::ErrorType
            | Self::MissingName
            | Self::MissingBody
            | Self::MissingTokenNode
            | Self::ErrorNode => IdentityClass::IdentityBearing,
        }
    }

    /// Returns the typed attachment family for an identity-bearing node.
    #[allow(
        clippy::too_many_lines,
        reason = "the wildcard-free typed-family table must enumerate every grammar kind explicitly"
    )]
    pub const fn ast_tag(self) -> Option<AstTag> {
        match self {
            Self::SourceFile => Some(AstTag::SourceFile),
            Self::ModuleDeclaration
            | Self::UseDeclaration
            | Self::FlowItem
            | Self::FunctionItem
            | Self::PredicateItem
            | Self::ProofItem
            | Self::TraitItem
            | Self::ImplItem
            | Self::EnumItem
            | Self::StructItem
            | Self::TypeAliasItem
            | Self::ResourceDeclarationItem
            | Self::CharacterDeclarationItem
            | Self::ViewDeclarationItem
            | Self::ActionDeclarationItem
            | Self::ActivityDeclarationItem
            | Self::SignalDeclarationItem
            | Self::MetricDeclarationItem
            | Self::LayerDeclarationItem
            | Self::EntryDeclarationItem
            | Self::ExternCapabilityItem
            | Self::TestItem
            | Self::BenchItem
            | Self::StyleItem
            | Self::ErrorItem => Some(AstTag::Item),
            Self::AssertionStatement
            | Self::LetStatement
            | Self::AssignmentStatement
            | Self::LetElseStatement
            | Self::LetChoiceStatement
            | Self::LetScopeStatement
            | Self::LetLoopStatement
            | Self::LetActionReceiveStatement
            | Self::ReturnStatement
            | Self::OutStatement
            | Self::GotoStatement
            | Self::DeferBlockStatement
            | Self::DeferStatement
            | Self::YieldStatement
            | Self::SignalStatement
            | Self::LifetimeSetStatement
            | Self::WaitStatement
            | Self::OnStatement
            | Self::UnsafeLifetimeStatement
            | Self::IfStatement
            | Self::LoopStatement
            | Self::WhileStatement
            | Self::WhileLetStatement
            | Self::ForStatement
            | Self::MatchStatement
            | Self::CloseStatement
            | Self::SelectStatement
            | Self::ChoiceStatement
            | Self::SourceLocaleStatement
            | Self::ScopeStatement
            | Self::IncludeStatement
            | Self::BreakStatement
            | Self::ContinueStatement
            | Self::ExpressionStatement
            | Self::ProofCallStatement
            | Self::ErrorStatement => Some(AstTag::Statement),
            Self::SelectBranch
            | Self::AwaitWithBranch
            | Self::ChoiceIfItem
            | Self::ChoiceIfBranch
            | Self::ChoiceForItem
            | Self::ChoiceMatchItem
            | Self::ChoiceMatchArm
            | Self::ChoiceOption
            | Self::ChoiceOptionFor
            | Self::ChoiceLabelField
            | Self::ChoiceIdField
            | Self::ChoiceValueField
            | Self::ChoiceVisibleField
            | Self::ChoiceEnabledField
            | Self::ChoiceOrderField
            | Self::ChoiceHotkeyField
            | Self::ChoiceViewField
            | Self::ChoiceSelectField
            | Self::ChoiceCompactArm
            | Self::ChoiceGotoAction
            | Self::ChoiceOutAction
            | Self::ChoicePlan
            | Self::ChoicePlanAssignment
            | Self::ChoicePlanTimeout
            | Self::ChoicePlanCancel
            | Self::ChoicePlanOnSelect
            | Self::InputTriggerPattern
            | Self::EventTriggerPattern
            | Self::SignalTriggerPattern
            | Self::TimeoutTriggerPattern
            | Self::MarkTriggerPattern
            | Self::SelectTriggerPattern
            | Self::TaskTriggerPattern
            | Self::ScopeTriggerPattern
            | Self::Visibility
            | Self::DeclarationHeader
            | Self::DeclarationPublicId
            | Self::SurfaceAlias
            | Self::GenericParameterGroup
            | Self::GenericParameter
            | Self::LifetimeParameter
            | Self::TypeParameter
            | Self::FixedParameterGroup
            | Self::Parameter
            | Self::WhereClause
            | Self::WherePredicate
            | Self::ReturnType
            | Self::RequiresClause
            | Self::EnsuresClause
            | Self::InvariantClause
            | Self::AssumeClause
            | Self::ReadsClause
            | Self::EffectsClause
            | Self::NoEffectClause
            | Self::ModifiesClause
            | Self::DecreasesClause
            | Self::ResourceFieldInitializer
            | Self::CharacterDisplayNameMember
            | Self::ViewExportBlock
            | Self::ViewExportDeclaration
            | Self::ViewFragment
            | Self::ActionSignature
            | Self::ActivityModeMember
            | Self::ActivityLifecycleMember
            | Self::ActivityInputBlock
            | Self::ActivityOutputBlock
            | Self::ActivityPort
            | Self::ActivityContractBlock
            | Self::SignalObservableType
            | Self::MetricKind
            | Self::MetricUnitMember
            | Self::MetricLabelsBlock
            | Self::MetricLabel
            | Self::MetricBucketsMember
            | Self::LayerKindNode
            | Self::LayerMember
            | Self::LayerPolicyValue
            | Self::RetainedReference
            | Self::StyleTokenDeclaration
            | Self::StyleRule
            | Self::StyleSelector
            | Self::StyleSelectorSequence
            | Self::StylePropertyDeclaration
            | Self::StyleEnvironmentBlock
            | Self::StyleEnvironmentCondition
            | Self::StyleEnvironmentClause
            | Self::EntryRoleBinding
            | Self::EntryGoto
            | Self::EntryRoute
            | Self::EntryRouteBinding
            | Self::EntryOption
            | Self::PostfixBracketPayload
            | Self::DialogueContent
            | Self::DialogueText
            | Self::DialogueRaw
            | Self::DialogueEscape
            | Self::DialogueRuby
            | Self::DialogueInterpolation
            | Self::DialogueControl
            | Self::DialogueMark
            | Self::DialogueLineBreak
            | Self::DialogueError => Some(AstTag::DeclarationPart),
            Self::LiteralExpression
            | Self::EntityReferenceExpression
            | Self::LifetimePathExpression
            | Self::PathExpression
            | Self::ShortVariantExpression
            | Self::PlaceholderExpression
            | Self::TupleExpression
            | Self::BracketSequenceExpression
            | Self::NumericBracketSequenceExpression
            | Self::ArrayRepeatExpression
            | Self::CallExpression
            | Self::SelectExpression
            | Self::PostfixBracketExpression
            | Self::DialogueContentApplicationExpression
            | Self::PipeExpression
            | Self::TryExpression
            | Self::AwaitExpression
            | Self::ThreadExpression
            | Self::ChoiceExpression
            | Self::RangeExpression
            | Self::RecordExpression
            | Self::RecordLiteralExpression
            | Self::BinaryExpression
            | Self::BorrowExpression
            | Self::DereferenceExpression
            | Self::ClosureExpression
            | Self::UnaryExpression
            | Self::BlockExpression
            | Self::ComputationBlockExpression
            | Self::NamedBlockExpression
            | Self::IfExpression
            | Self::IfLetExpression
            | Self::MatchExpression
            | Self::MatchArm
            | Self::CallArgument
            | Self::RecordField
            | Self::ClosureParameter
            | Self::OmittedBlockTail
            | Self::MissingExpression
            | Self::ErrorExpression => Some(AstTag::Expression),
            Self::RichTextTag
            | Self::RichTextEndTag
            | Self::RichTextTagName
            | Self::RichTextArgumentPayload
            | Self::RichTextFxCallPayload
            | Self::RichTextDialogueCallPayload
            | Self::RichTextConditionPayload
            | Self::RichTextPositionalArgument
            | Self::RichTextNamedArgument
            | Self::RichTextInvalidArgument
            | Self::RichTextArgumentKey
            | Self::RichTextArgumentEquals
            | Self::RichTextArgumentValue
            | Self::RichTextArgumentToken
            | Self::RichTextArgumentContent
            | Self::RichTextArgumentQuote
            | Self::RichTextMissingArgumentValue
            | Self::RichTextInvalidArgumentIssue => Some(AstTag::RichText),
            Self::WildcardPattern
            | Self::BindingPattern
            | Self::MutableBindingPattern
            | Self::TypedBindingPattern
            | Self::LiteralPattern
            | Self::EntityReferencePattern
            | Self::TuplePattern
            | Self::RecordPattern
            | Self::RecordPatternField
            | Self::VariantPattern
            | Self::SequencePattern
            | Self::RestPattern
            | Self::WholeBindingPattern
            | Self::OrPattern
            | Self::MissingPattern
            | Self::ErrorPattern => Some(AstTag::Pattern),
            Self::PrimitiveType
            | Self::PathType
            | Self::GenericApplicationType
            | Self::TupleType
            | Self::ReferenceType
            | Self::SliceType
            | Self::FunctionType
            | Self::SumType
            | Self::TypeArgument
            | Self::MissingType
            | Self::ErrorType => Some(AstTag::Type),
            Self::InnerAttribute | Self::OuterAttribute | Self::DocBlock => Some(AstTag::Attribute),
            Self::NameDefinition | Self::NameReference | Self::MissingName => Some(AstTag::Name),
            Self::Path => Some(AstTag::Path),
            Self::ExpressionBody
            | Self::PredicateBody
            | Self::ProofBody
            | Self::FunctionBody
            | Self::FlowBody
            | Self::ResourceBody
            | Self::CharacterBody
            | Self::ViewDeclarationBody
            | Self::ActivityBody
            | Self::MetricBody
            | Self::LayerBody
            | Self::StyleBody
            | Self::EntryBody
            | Self::Block
            | Self::ChoiceBody
            | Self::ChoiceOptionBody
            | Self::ChoiceViewBody
            | Self::ChoicePlanBody
            | Self::PredicateBlock
            | Self::ProofBlock => Some(AstTag::Body),
            Self::OpenBraceNode
            | Self::CloseBraceNode
            | Self::OpenParenNode
            | Self::CloseParenNode
            | Self::OpenBracketNode
            | Self::CloseBracketNode
            | Self::OpenAngleNode
            | Self::CloseAngleNode
            | Self::EqualsNode
            | Self::ColonNode
            | Self::ThinArrowNode
            | Self::ForInNode
            | Self::RestParameterMarker => Some(AstTag::Delimiter),
            Self::MissingBody
            | Self::MissingTokenNode
            | Self::ErrorDeclarationMember
            | Self::WrongFamilyReference
            | Self::MissingDeclarationId
            | Self::MissingMemberValue
            | Self::ErrorNode => Some(AstTag::Recovery),
            Self::ItemList
            | Self::StatementList
            | Self::ExpressionList
            | Self::ParameterList
            | Self::GenericParameterList
            | Self::WherePredicateList
            | Self::AttributeList
            | Self::FieldList
            | Self::ArgumentList
            | Self::RichTextArgumentList
            | Self::MatchArmList
            | Self::LogicalLine
            | Self::IndentedSuite
            | Self::FenceBody
            | Self::DelimitedGroup
            | Self::PathSegment
            | Self::WhitespaceToken
            | Self::NewlineToken
            | Self::CommentToken
            | Self::DocCommentToken
            | Self::IdentifierToken
            | Self::LifetimeToken
            | Self::NumberToken
            | Self::StringToken
            | Self::UnterminatedStringToken
            | Self::RawStringToken
            | Self::CharacterToken
            | Self::EntityReferenceToken
            | Self::KeywordToken
            | Self::PunctuationToken
            | Self::TextToken
            | Self::ErrorToken
            | Self::MissingToken
            | Self::EofToken => None,
        }
    }

    pub(crate) const fn is_item(self) -> bool {
        matches!(
            self,
            Self::ModuleDeclaration
                | Self::UseDeclaration
                | Self::FlowItem
                | Self::FunctionItem
                | Self::PredicateItem
                | Self::ProofItem
                | Self::TraitItem
                | Self::ImplItem
                | Self::EnumItem
                | Self::StructItem
                | Self::TypeAliasItem
                | Self::ResourceDeclarationItem
                | Self::CharacterDeclarationItem
                | Self::ViewDeclarationItem
                | Self::ActionDeclarationItem
                | Self::ActivityDeclarationItem
                | Self::SignalDeclarationItem
                | Self::MetricDeclarationItem
                | Self::LayerDeclarationItem
                | Self::EntryDeclarationItem
                | Self::ExternCapabilityItem
                | Self::TestItem
                | Self::BenchItem
                | Self::StyleItem
                | Self::ErrorItem
        )
    }

    /// Returns whether this node is one of the maintained callable contract
    /// clause families. The heterogeneous parent role owns source order.
    pub(crate) const fn is_contract_clause(self) -> bool {
        matches!(
            self,
            Self::RequiresClause
                | Self::EnsuresClause
                | Self::InvariantClause
                | Self::AssumeClause
                | Self::ReadsClause
                | Self::EffectsClause
                | Self::NoEffectClause
                | Self::ModifiesClause
                | Self::DecreasesClause
        )
    }

    pub(crate) const fn is_statement(self) -> bool {
        matches!(
            self,
            Self::AssertionStatement
                | Self::LetStatement
                | Self::AssignmentStatement
                | Self::LetElseStatement
                | Self::LetChoiceStatement
                | Self::LetScopeStatement
                | Self::LetLoopStatement
                | Self::LetActionReceiveStatement
                | Self::ReturnStatement
                | Self::OutStatement
                | Self::GotoStatement
                | Self::DeferBlockStatement
                | Self::DeferStatement
                | Self::YieldStatement
                | Self::SignalStatement
                | Self::LifetimeSetStatement
                | Self::WaitStatement
                | Self::OnStatement
                | Self::UnsafeLifetimeStatement
                | Self::IfStatement
                | Self::LoopStatement
                | Self::WhileStatement
                | Self::WhileLetStatement
                | Self::ForStatement
                | Self::MatchStatement
                | Self::CloseStatement
                | Self::SelectStatement
                | Self::ChoiceStatement
                | Self::SourceLocaleStatement
                | Self::ScopeStatement
                | Self::IncludeStatement
                | Self::BreakStatement
                | Self::ContinueStatement
                | Self::ExpressionStatement
                | Self::ProofCallStatement
                | Self::ErrorStatement
        )
    }

    /// Whether this node may be one direct child of a statement-only Flow or
    /// Thread body. Attachment applies the stricter sixteen-family split.
    pub(crate) const fn is_thread_flow_item(self) -> bool {
        self.is_statement() || matches!(self, Self::DialogueContentApplicationExpression)
    }

    pub(crate) const fn is_expression(self) -> bool {
        matches!(
            self,
            Self::LiteralExpression
                | Self::EntityReferenceExpression
                | Self::LifetimePathExpression
                | Self::PathExpression
                | Self::ShortVariantExpression
                | Self::PlaceholderExpression
                | Self::TupleExpression
                | Self::BracketSequenceExpression
                | Self::NumericBracketSequenceExpression
                | Self::ArrayRepeatExpression
                | Self::CallExpression
                | Self::SelectExpression
                | Self::PostfixBracketExpression
                | Self::DialogueContentApplicationExpression
                | Self::PipeExpression
                | Self::TryExpression
                | Self::AwaitExpression
                | Self::ThreadExpression
                | Self::ChoiceExpression
                | Self::RangeExpression
                | Self::RecordExpression
                | Self::RecordLiteralExpression
                | Self::BinaryExpression
                | Self::BorrowExpression
                | Self::DereferenceExpression
                | Self::ClosureExpression
                | Self::UnaryExpression
                | Self::BlockExpression
                | Self::ComputationBlockExpression
                | Self::NamedBlockExpression
                | Self::IfExpression
                | Self::IfLetExpression
                | Self::MatchExpression
                | Self::MissingExpression
                | Self::ErrorExpression
        )
    }

    pub(crate) const fn is_pattern_node(self) -> bool {
        matches!(
            self,
            Self::WildcardPattern
                | Self::BindingPattern
                | Self::MutableBindingPattern
                | Self::TypedBindingPattern
                | Self::LiteralPattern
                | Self::EntityReferencePattern
                | Self::TuplePattern
                | Self::RecordPattern
                | Self::RecordPatternField
                | Self::VariantPattern
                | Self::SequencePattern
                | Self::RestPattern
                | Self::WholeBindingPattern
                | Self::OrPattern
                | Self::MissingPattern
                | Self::ErrorPattern
        )
    }

    pub(crate) const fn is_type_node(self) -> bool {
        matches!(
            self,
            Self::PrimitiveType
                | Self::PathType
                | Self::GenericApplicationType
                | Self::TupleType
                | Self::ReferenceType
                | Self::SliceType
                | Self::FunctionType
                | Self::SumType
                | Self::MissingType
                | Self::ErrorType
        )
    }

    pub(crate) const fn is_retained_declaration_member(self) -> bool {
        matches!(
            self,
            Self::CharacterDisplayNameMember
                | Self::ActivityModeMember
                | Self::ActivityLifecycleMember
                | Self::ActivityInputBlock
                | Self::ActivityOutputBlock
                | Self::ActivityContractBlock
                | Self::MetricUnitMember
                | Self::MetricLabelsBlock
                | Self::MetricBucketsMember
                | Self::LayerMember
        )
    }

    /// Whether this identity-bearing node represents a missing grammar value.
    pub(crate) const fn is_missing_node(self) -> bool {
        matches!(
            self,
            Self::MissingName
                | Self::MissingBody
                | Self::MissingTokenNode
                | Self::MissingExpression
                | Self::MissingPattern
                | Self::MissingType
                | Self::RichTextMissingArgumentValue
        )
    }

    /// Whether this identity-bearing node owns ordinary current-grammar error recovery.
    pub(crate) const fn is_error_node(self) -> bool {
        matches!(
            self,
            Self::ErrorItem
                | Self::ErrorDeclarationMember
                | Self::ErrorStatement
                | Self::ErrorExpression
                | Self::ErrorPattern
                | Self::ErrorType
                | Self::RichTextInvalidArgument
                | Self::RichTextInvalidArgumentIssue
                | Self::ErrorNode
        )
    }

    /// Whether this node is a deliberate zero-width omitted block tail.
    pub(crate) const fn is_omitted_node(self) -> bool {
        matches!(self, Self::OmittedBlockTail)
    }
}

#[cfg(test)]
mod tests {
    use super::{AstTag, IdentityClass, SyntaxKind};

    #[test]
    fn final_kind_inventory_owns_identity_classification() {
        assert_eq!(
            SyntaxKind::SourceFile.identity_class(),
            IdentityClass::IdentityBearing
        );
        assert_eq!(
            SyntaxKind::PathSegment.identity_class(),
            IdentityClass::StructuralWrapper
        );
        assert_eq!(SyntaxKind::PathSegment.ast_tag(), None);
        assert_eq!(
            SyntaxKind::ProofBlock.identity_class(),
            IdentityClass::IdentityBearing
        );
        assert!(SyntaxKind::MissingToken.is_token());
        assert!(!SyntaxKind::MissingTokenNode.is_token());
    }

    #[test]
    fn typed_attachment_tags_cover_semantic_families_only() {
        assert_eq!(SyntaxKind::SourceFile.ast_tag(), Some(AstTag::SourceFile));
        assert_eq!(SyntaxKind::ProofItem.ast_tag(), Some(AstTag::Item));
        assert_eq!(
            SyntaxKind::DialogueContentApplicationExpression.ast_tag(),
            Some(AstTag::Expression)
        );
        assert_eq!(
            SyntaxKind::RecordPatternField.ast_tag(),
            Some(AstTag::Pattern)
        );
        assert_eq!(SyntaxKind::TypeArgument.ast_tag(), Some(AstTag::Type));
        assert_eq!(
            SyntaxKind::MissingTokenNode.ast_tag(),
            Some(AstTag::Recovery)
        );
        assert_eq!(SyntaxKind::ItemList.ast_tag(), None);
        assert_eq!(SyntaxKind::IdentifierToken.ast_tag(), None);
    }

    #[test]
    fn complete_kind_inventory_aligns_identity_and_typed_attachment() {
        assert_eq!(
            SyntaxKind::ALL.len(),
            SyntaxKind::EofToken as usize + 1,
            "the macro-owned inventory must contain every discriminant"
        );
        for (ordinal, &kind) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(kind as usize, ordinal, "inventory order for {kind:?}");
            assert_eq!(
                kind.ast_tag().is_some(),
                kind.identity_class() == IdentityClass::IdentityBearing,
                "typed attachment ownership for {kind:?}"
            );
        }
    }
}
