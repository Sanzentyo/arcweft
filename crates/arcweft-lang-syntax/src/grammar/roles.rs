//! Semantic child roles for grammar attachment and reconciliation.

/// Closed Activity policy spelling selected by the one-pass declaration parser.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ActivityPolicySyntaxValue {
    ModeDeterministic,
    ModeCheckpointedRealtime,
    ModeExternalRealtime,
    LifecycleStateless,
    LifecycleSnapshot,
}

/// Closed Metric kind selected by the one-pass declaration parser.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MetricKindSyntaxValue {
    Counter,
    Gauge,
    Histogram,
}

/// Closed Layer kind selected by the one-pass declaration parser.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LayerKindSyntaxValue {
    Background,
    World2d,
    Character,
    Effects,
    Dialogue,
    GameView,
    HtmlView,
    Activity,
    Modal,
    Overlay,
    Debug,
    Agent,
    Offscreen,
    Custom,
}

/// Closed Layer member name selected by the one-pass declaration parser.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LayerMemberSyntaxKind {
    Parent,
    Phase,
    Z,
    Visible,
    Transform,
    Input,
    HitTest,
    Capture,
    Accessibility,
    View,
    Activity,
}

impl LayerMemberSyntaxKind {
    pub(crate) const fn spelling(self) -> &'static str {
        match self {
            Self::Parent => "parent",
            Self::Phase => "phase",
            Self::Z => "z",
            Self::Visible => "visible",
            Self::Transform => "transform",
            Self::Input => "input",
            Self::HitTest => "hit_test",
            Self::Capture => "capture",
            Self::Accessibility => "accessibility",
            Self::View => "view",
            Self::Activity => "activity",
        }
    }
}

/// Closed Layer policy spelling selected by the one-pass declaration parser.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LayerPolicySyntaxValue {
    PhaseBackground,
    PhaseWorld,
    PhaseCharacters,
    PhaseEffects,
    PhaseDialogue,
    PhaseGameView,
    PhaseHtmlView,
    PhaseModal,
    PhaseDebug,
    PhaseAgentOverlay,
    InputIgnore,
    InputPassThrough,
    InputHitTest,
    InputModal,
    InputCapture,
    HitTestNone,
    HitTestBounds,
    HitTestViewTree,
    HitTestObjectIdMask,
    CaptureNone,
    CaptureColor,
    CaptureObjectId,
    CaptureMask,
    CaptureAll,
    AccessibilityHidden,
    AccessibilityExposed,
    AccessibilityContainer,
}

/// Semantic child role used when reconciling identity-bearing grammar nodes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRole {
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
    ContractClause(u16),
    ContractMode,
    ContractOperand(u16),
    Body,
    OpenDelimiter,
    CloseDelimiter,
    Statement(u32),
    ThreadFlowItem(u32),
    ChoiceItem(u32),
    ChoicePlanItem(u32),
    ChoiceOptionField(u32),
    ChoiceViewField(u32),
    Branch(u32),
    TrailingRecovery(u32),
    Tail,
    Condition,
    Callee,
    Argument(u16),
    DialogueNode(u32),
    RichTextTag(u32),
    Payload,
    Key,
    Equals,
    Colon,
    Value,
    Token,
    Content,
    Plan,
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
    MatchArm(u32),
    Field(u16),
    Member(u16),
    InputPort(u16),
    OutputPort(u16),
    Export(u16),
    Label(u16),
    Bucket(u16),
    Policy(u16),
    ActivityPolicyValue(ActivityPolicySyntaxValue),
    MetricKindValue(MetricKindSyntaxValue),
    LayerKindValue(LayerKindSyntaxValue),
    LayerMemberName(LayerMemberSyntaxKind),
    LayerPolicyValue(LayerPolicySyntaxValue),
    Reference(u16),
    RelatedReference(u16),
    Element(u32),
    Recovery(u32),
}

/// Ordinal-free semantic child role used as reconciliation authority.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRoleClass {
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
    ContractClause,
    ContractMode,
    ContractOperand,
    Body,
    OpenDelimiter,
    CloseDelimiter,
    Statement,
    ThreadFlowItem,
    ChoiceItem,
    ChoicePlanItem,
    ChoiceOptionField,
    ChoiceViewField,
    Branch,
    TrailingRecovery,
    Tail,
    Condition,
    Callee,
    Argument,
    DialogueNode,
    RichTextTag,
    Payload,
    Key,
    Equals,
    Colon,
    Value,
    Token,
    Content,
    Plan,
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
    pub const fn class(self) -> SyntaxRoleClass {
        match self {
            Self::Root => SyntaxRoleClass::Root,
            Self::Attribute(_) => SyntaxRoleClass::Attribute,
            Self::Documentation => SyntaxRoleClass::Documentation,
            Self::Visibility => SyntaxRoleClass::Visibility,
            Self::PublicId => SyntaxRoleClass::PublicId,
            Self::Alias => SyntaxRoleClass::Alias,
            Self::Kind | Self::MetricKindValue(_) => SyntaxRoleClass::Kind,
            Self::Name | Self::LayerMemberName(_) => SyntaxRoleClass::Name,
            Self::GenericGroup => SyntaxRoleClass::GenericGroup,
            Self::GenericParameter(_) => SyntaxRoleClass::GenericParameter,
            Self::ParameterGroup => SyntaxRoleClass::ParameterGroup,
            Self::Parameter(_) => SyntaxRoleClass::Parameter,
            Self::ParameterPattern => SyntaxRoleClass::ParameterPattern,
            Self::ParameterType => SyntaxRoleClass::ParameterType,
            Self::WhereClause => SyntaxRoleClass::WhereClause,
            Self::WherePredicate(_) => SyntaxRoleClass::WherePredicate,
            Self::ReturnType => SyntaxRoleClass::ReturnType,
            Self::ContractClause(_) => SyntaxRoleClass::ContractClause,
            Self::ContractMode => SyntaxRoleClass::ContractMode,
            Self::ContractOperand(_) => SyntaxRoleClass::ContractOperand,
            Self::Body => SyntaxRoleClass::Body,
            Self::OpenDelimiter => SyntaxRoleClass::OpenDelimiter,
            Self::CloseDelimiter => SyntaxRoleClass::CloseDelimiter,
            Self::Statement(_) => SyntaxRoleClass::Statement,
            Self::ThreadFlowItem(_) => SyntaxRoleClass::ThreadFlowItem,
            Self::ChoiceItem(_) => SyntaxRoleClass::ChoiceItem,
            Self::ChoicePlanItem(_) => SyntaxRoleClass::ChoicePlanItem,
            Self::ChoiceOptionField(_) => SyntaxRoleClass::ChoiceOptionField,
            Self::ChoiceViewField(_) => SyntaxRoleClass::ChoiceViewField,
            Self::Branch(_) => SyntaxRoleClass::Branch,
            Self::TrailingRecovery(_) => SyntaxRoleClass::TrailingRecovery,
            Self::Tail => SyntaxRoleClass::Tail,
            Self::Condition => SyntaxRoleClass::Condition,
            Self::Callee => SyntaxRoleClass::Callee,
            Self::Argument(_) => SyntaxRoleClass::Argument,
            Self::DialogueNode(_) => SyntaxRoleClass::DialogueNode,
            Self::RichTextTag(_) => SyntaxRoleClass::RichTextTag,
            Self::Payload => SyntaxRoleClass::Payload,
            Self::Key => SyntaxRoleClass::Key,
            Self::Equals => SyntaxRoleClass::Equals,
            Self::Colon => SyntaxRoleClass::Colon,
            Self::Value
            | Self::ActivityPolicyValue(_)
            | Self::LayerKindValue(_)
            | Self::LayerPolicyValue(_) => SyntaxRoleClass::Value,
            Self::Token => SyntaxRoleClass::Token,
            Self::Content => SyntaxRoleClass::Content,
            Self::Plan => SyntaxRoleClass::Plan,
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
    pub const fn ordinal(self) -> Option<u32> {
        match self {
            Self::Attribute(ordinal)
            | Self::GenericParameter(ordinal)
            | Self::Parameter(ordinal)
            | Self::WherePredicate(ordinal)
            | Self::ContractClause(ordinal)
            | Self::ContractOperand(ordinal)
            | Self::Argument(ordinal)
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
            | Self::ThreadFlowItem(ordinal)
            | Self::ChoiceItem(ordinal)
            | Self::ChoicePlanItem(ordinal)
            | Self::ChoiceOptionField(ordinal)
            | Self::ChoiceViewField(ordinal)
            | Self::Branch(ordinal)
            | Self::TrailingRecovery(ordinal)
            | Self::DialogueNode(ordinal)
            | Self::RichTextTag(ordinal)
            | Self::MatchArm(ordinal)
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
            | Self::ContractMode
            | Self::Body
            | Self::OpenDelimiter
            | Self::CloseDelimiter
            | Self::Tail
            | Self::Condition
            | Self::Callee
            | Self::Payload
            | Self::Key
            | Self::Equals
            | Self::Colon
            | Self::Value
            | Self::Token
            | Self::Content
            | Self::Plan
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
            | Self::ElseBranch
            | Self::ActivityPolicyValue(_)
            | Self::MetricKindValue(_)
            | Self::LayerKindValue(_)
            | Self::LayerMemberName(_)
            | Self::LayerPolicyValue(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LayerKindSyntaxValue, LayerMemberSyntaxKind, LayerPolicySyntaxValue, MetricKindSyntaxValue,
        SyntaxRole, SyntaxRoleClass,
    };

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
        assert_eq!(SyntaxRole::ContractClause(4).ordinal(), Some(4));
        assert_eq!(SyntaxRole::ContractOperand(2).ordinal(), Some(2));
        assert_eq!(SyntaxRole::ThreadFlowItem(3).ordinal(), Some(3));
        assert_eq!(
            SyntaxRole::ChoiceOptionField(u32::from(u16::MAX) + 1).ordinal(),
            Some(u32::from(u16::MAX) + 1)
        );
        assert_eq!(
            SyntaxRole::ChoiceViewField(u32::from(u16::MAX) + 1).ordinal(),
            Some(u32::from(u16::MAX) + 1)
        );
        assert_eq!(SyntaxRole::TrailingRecovery(5).ordinal(), Some(5));
        assert_eq!(SyntaxRole::ContractMode.ordinal(), None);
        assert_eq!(SyntaxRole::Element(42).ordinal(), Some(42));
        assert_eq!(
            SyntaxRole::MatchArm(u32::from(u16::MAX) + 1).ordinal(),
            Some(u32::from(u16::MAX) + 1)
        );
        assert_eq!(SyntaxRole::Condition.ordinal(), None);
        assert_eq!(
            SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Histogram).class(),
            SyntaxRoleClass::Kind
        );
        assert_eq!(
            SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Counter).ordinal(),
            None
        );
        assert_eq!(
            SyntaxRole::LayerKindValue(LayerKindSyntaxValue::Dialogue).class(),
            SyntaxRoleClass::Value
        );
        assert_eq!(
            SyntaxRole::LayerMemberName(LayerMemberSyntaxKind::HitTest).class(),
            SyntaxRoleClass::Name
        );
        assert_eq!(
            SyntaxRole::LayerPolicyValue(LayerPolicySyntaxValue::CaptureAll).ordinal(),
            None
        );
    }
}
