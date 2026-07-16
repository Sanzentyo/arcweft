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
    EntryDeclarationItem,
    ExternCapabilityItem,
    ExternModuleItem,
    DialogueDefaultsItem,
    TestItem,
    BenchItem,
    SourceItem,
    StyleItem,
    TopLevelFlowItem,
    ErrorItem,
    InnerAttribute,
    OuterAttribute,
    DocBlock,
    Visibility,
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
    Element(u32),
    Recovery(u32),
}

#[cfg(test)]
mod tests {
    use super::{IdentityClass, SyntaxKind};

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
}
