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
    }
}
