//! Semantic child roles for grammar attachment and reconciliation.

/// Semantic child role used when reconciling identity-bearing grammar nodes.
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
    RichTextTag(u32),
    Payload,
    Key,
    Equals,
    Value,
    Token,
    Content,
    OpeningQuote,
    ClosingQuote,
    Issue,
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
    #[allow(
        dead_code,
        reason = "the accepted grammar contract reserves related references for attached cross-links"
    )]
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
    RichTextTag,
    Payload,
    Key,
    Equals,
    Value,
    Token,
    Content,
    OpeningQuote,
    ClosingQuote,
    Issue,
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
            Self::RichTextTag(_) => SyntaxRoleClass::RichTextTag,
            Self::Payload => SyntaxRoleClass::Payload,
            Self::Key => SyntaxRoleClass::Key,
            Self::Equals => SyntaxRoleClass::Equals,
            Self::Value => SyntaxRoleClass::Value,
            Self::Token => SyntaxRoleClass::Token,
            Self::Content => SyntaxRoleClass::Content,
            Self::OpeningQuote => SyntaxRoleClass::OpeningQuote,
            Self::ClosingQuote => SyntaxRoleClass::ClosingQuote,
            Self::Issue => SyntaxRoleClass::Issue,
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

    /// Returns the deterministic sibling ordinal carried by an ordered role.
    #[allow(
        dead_code,
        reason = "private ordered attached-child access precedes the atomic ParsedSource syntax switch"
    )]
    pub(crate) const fn ordinal(self) -> Option<u32> {
        match self {
            Self::Attribute(ordinal)
            | Self::GenericParameter(ordinal)
            | Self::Parameter(ordinal)
            | Self::WherePredicate(ordinal)
            | Self::RequiresClause(ordinal)
            | Self::EnsuresClause(ordinal)
            | Self::Argument(ordinal)
            | Self::MatchArm(ordinal)
            | Self::Field(ordinal)
            | Self::Member(ordinal)
            | Self::InputPort(ordinal)
            | Self::OutputPort(ordinal)
            | Self::Export(ordinal)
            | Self::Label(ordinal)
            | Self::Bucket(ordinal)
            | Self::Policy(ordinal)
            | Self::Reference(ordinal)
            | Self::RelatedReference(ordinal) => Some(ordinal as u32),
            Self::Statement(ordinal)
            | Self::RichTextTag(ordinal)
            | Self::Element(ordinal)
            | Self::Recovery(ordinal) => Some(ordinal),
            Self::Root
            | Self::Documentation
            | Self::Visibility
            | Self::PublicId
            | Self::Alias
            | Self::Kind
            | Self::Name
            | Self::GenericGroup
            | Self::ParameterGroup
            | Self::ParameterPattern
            | Self::ParameterType
            | Self::WhereClause
            | Self::ReturnType
            | Self::Body
            | Self::OpenDelimiter
            | Self::CloseDelimiter
            | Self::Tail
            | Self::Condition
            | Self::Callee
            | Self::Payload
            | Self::Key
            | Self::Equals
            | Self::Value
            | Self::Token
            | Self::Content
            | Self::OpeningQuote
            | Self::ClosingQuote
            | Self::Issue
            | Self::Target
            | Self::Operand
            | Self::LeftOperand
            | Self::RightOperand
            | Self::Pattern
            | Self::Type
            | Self::Initializer
            | Self::Scrutinee
            | Self::Guard
            | Self::ThenBranch
            | Self::ElseBranch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SyntaxRole, SyntaxRoleClass};

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
        assert_eq!(SyntaxRole::Argument(9).ordinal(), Some(9));
        assert_eq!(SyntaxRole::Element(42).ordinal(), Some(42));
        assert_eq!(SyntaxRole::Condition.ordinal(), None);
    }
}
