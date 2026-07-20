//! Final grammar-node and token vocabulary for the staged lossless parser.

/// Grammar node and token kinds produced by the final event parser.
///
/// Raw Rowan conversion remains private until the public syntax switch. Tokens
/// never receive syntax identity; structural wrappers retain layout without
/// becoming a second semantic-parent authority.
#[allow(
    dead_code,
    reason = "consumed by the staged shadow grammar in the next cut"
)]
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SyntaxKind {
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
    WhereClause,
    WherePredicate,
    ReturnType,
    RequiresClause,
    EnsuresClause,
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
    AssertionStatement,
    LetStatement,
    AssignmentStatement,
    LetElseStatement,
    LetChoiceStatement,
    LetScopeStatement,
    LetLoopStatement,
    LetAwaitStatement,
    LetActionReceiveStatement,
    ReturnStatement,
    OutStatement,
    GotoStatement,
    ThreadStatement,
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
    DialogueCallExpression,
    IndexExpression,
    PipeExpression,
    TryExpression,
    AwaitExpression,
    ThreadExpression,
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
    ArrayType,
    FunctionType,
    SumType,
    InferType,
    LifetimeType,
    ElidedRegionType,
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
#[allow(
    dead_code,
    reason = "consumed by the staged shadow grammar in the next cut"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum IdentityClass {
    IdentityBearing,
    StructuralWrapper,
    Token,
}

impl SyntaxKind {
    /// Returns whether this kind is a token rather than a Rowan node.
    #[allow(
        dead_code,
        reason = "consumed by the staged shadow grammar in the next cut"
    )]
    pub(crate) const fn is_token(self) -> bool {
        matches!(self.identity_class(), IdentityClass::Token)
    }

    /// Returns the identity policy owned by this grammar kind.
    #[allow(
        dead_code,
        reason = "consumed by the staged shadow grammar in the next cut"
    )]
    pub(crate) const fn identity_class(self) -> IdentityClass {
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
            | Self::RawStringToken
            | Self::CharacterToken
            | Self::EntityReferenceToken
            | Self::KeywordToken
            | Self::PunctuationToken
            | Self::TextToken
            | Self::ErrorToken
            | Self::MissingToken
            | Self::EofToken => IdentityClass::Token,
            _ => IdentityClass::IdentityBearing,
        }
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
                | Self::ErrorNode
        )
    }

    /// Whether this node is a deliberate zero-width omitted block tail.
    pub(crate) const fn is_omitted_node(self) -> bool {
        matches!(self, Self::OmittedBlockTail)
    }
}

/// Semantic child role used when reconciling identity-bearing grammar nodes.
#[allow(
    dead_code,
    reason = "consumed by the staged shadow grammar in the next cut"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SyntaxRole {
    Root,
    Attribute(u16),
    Documentation,
    Visibility,
    PublicId,
    Alias,
    Kind,
    Name,
    GenericGroup,
    GenericParameter(u16),
    ParameterGroup,
    Parameter(u16),
    ParameterPattern,
    ParameterType,
    WhereClause,
    WherePredicate(u16),
    ReturnType,
    RequiresClause(u16),
    EnsuresClause(u16),
    Body,
    OpenDelimiter,
    CloseDelimiter,
    Statement(u32),
    Tail,
    Condition,
    Callee,
    Argument(u16),
    Target,
    Operand,
    LeftOperand,
    RightOperand,
    Pattern,
    Type,
    Initializer,
    Scrutinee,
    Guard,
    ThenBranch,
    ElseBranch,
    MatchArm(u16),
    Field(u16),
    Member(u16),
    InputPort(u16),
    OutputPort(u16),
    Export(u16),
    Label(u16),
    Bucket(u16),
    Policy(u16),
    Reference(u16),
    RelatedReference(u16),
    Element(u32),
    Recovery(u32),
}

/// Ordinal-free semantic child role used as reconciliation authority.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SyntaxRoleClass {
    Root,
    Attribute,
    Documentation,
    Visibility,
    PublicId,
    Alias,
    Kind,
    Name,
    GenericGroup,
    GenericParameter,
    ParameterGroup,
    Parameter,
    ParameterPattern,
    ParameterType,
    WhereClause,
    WherePredicate,
    ReturnType,
    RequiresClause,
    EnsuresClause,
    Body,
    OpenDelimiter,
    CloseDelimiter,
    Statement,
    Tail,
    Condition,
    Callee,
    Argument,
    Target,
    Operand,
    LeftOperand,
    RightOperand,
    Pattern,
    Type,
    Initializer,
    Scrutinee,
    Guard,
    ThenBranch,
    ElseBranch,
    MatchArm,
    Field,
    Member,
    InputPort,
    OutputPort,
    Export,
    Label,
    Bucket,
    Policy,
    Reference,
    RelatedReference,
    Element,
    Recovery,
}

impl SyntaxRole {
    /// Removes the deterministic sibling ordinal without weakening the role.
    pub(crate) const fn class(self) -> SyntaxRoleClass {
        match self {
            Self::Root => SyntaxRoleClass::Root,
            Self::Attribute(_) => SyntaxRoleClass::Attribute,
            Self::Documentation => SyntaxRoleClass::Documentation,
            Self::Visibility => SyntaxRoleClass::Visibility,
            Self::PublicId => SyntaxRoleClass::PublicId,
            Self::Alias => SyntaxRoleClass::Alias,
            Self::Kind => SyntaxRoleClass::Kind,
            Self::Name => SyntaxRoleClass::Name,
            Self::GenericGroup => SyntaxRoleClass::GenericGroup,
            Self::GenericParameter(_) => SyntaxRoleClass::GenericParameter,
            Self::ParameterGroup => SyntaxRoleClass::ParameterGroup,
            Self::Parameter(_) => SyntaxRoleClass::Parameter,
            Self::ParameterPattern => SyntaxRoleClass::ParameterPattern,
            Self::ParameterType => SyntaxRoleClass::ParameterType,
            Self::WhereClause => SyntaxRoleClass::WhereClause,
            Self::WherePredicate(_) => SyntaxRoleClass::WherePredicate,
            Self::ReturnType => SyntaxRoleClass::ReturnType,
            Self::RequiresClause(_) => SyntaxRoleClass::RequiresClause,
            Self::EnsuresClause(_) => SyntaxRoleClass::EnsuresClause,
            Self::Body => SyntaxRoleClass::Body,
            Self::OpenDelimiter => SyntaxRoleClass::OpenDelimiter,
            Self::CloseDelimiter => SyntaxRoleClass::CloseDelimiter,
            Self::Statement(_) => SyntaxRoleClass::Statement,
            Self::Tail => SyntaxRoleClass::Tail,
            Self::Condition => SyntaxRoleClass::Condition,
            Self::Callee => SyntaxRoleClass::Callee,
            Self::Argument(_) => SyntaxRoleClass::Argument,
            Self::Target => SyntaxRoleClass::Target,
            Self::Operand => SyntaxRoleClass::Operand,
            Self::LeftOperand => SyntaxRoleClass::LeftOperand,
            Self::RightOperand => SyntaxRoleClass::RightOperand,
            Self::Pattern => SyntaxRoleClass::Pattern,
            Self::Type => SyntaxRoleClass::Type,
            Self::Initializer => SyntaxRoleClass::Initializer,
            Self::Scrutinee => SyntaxRoleClass::Scrutinee,
            Self::Guard => SyntaxRoleClass::Guard,
            Self::ThenBranch => SyntaxRoleClass::ThenBranch,
            Self::ElseBranch => SyntaxRoleClass::ElseBranch,
            Self::MatchArm(_) => SyntaxRoleClass::MatchArm,
            Self::Field(_) => SyntaxRoleClass::Field,
            Self::Member(_) => SyntaxRoleClass::Member,
            Self::InputPort(_) => SyntaxRoleClass::InputPort,
            Self::OutputPort(_) => SyntaxRoleClass::OutputPort,
            Self::Export(_) => SyntaxRoleClass::Export,
            Self::Label(_) => SyntaxRoleClass::Label,
            Self::Bucket(_) => SyntaxRoleClass::Bucket,
            Self::Policy(_) => SyntaxRoleClass::Policy,
            Self::Reference(_) => SyntaxRoleClass::Reference,
            Self::RelatedReference(_) => SyntaxRoleClass::RelatedReference,
            Self::Element(_) => SyntaxRoleClass::Element,
            Self::Recovery(_) => SyntaxRoleClass::Recovery,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IdentityClass, SyntaxKind, SyntaxRole, SyntaxRoleClass};

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
        assert_eq!(
            SyntaxKind::ProofBlock.identity_class(),
            IdentityClass::IdentityBearing
        );
        assert!(SyntaxKind::MissingToken.is_token());
        assert!(!SyntaxKind::MissingTokenNode.is_token());
    }

    #[test]
    fn role_classes_discard_only_sibling_ordinals() {
        assert_eq!(SyntaxRole::Statement(7).class(), SyntaxRoleClass::Statement);
        assert_eq!(
            SyntaxRole::Statement(99).class(),
            SyntaxRoleClass::Statement
        );
        assert_ne!(
            SyntaxRole::Parameter(0).class(),
            SyntaxRole::ParameterType.class()
        );
    }
}
