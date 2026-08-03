//! Typed full options, View fields, selections, and compact Choice arms.

use super::super::AttachedPatternNode;
use super::super::access::RequiredStatementExpressionNode;
use super::super::family::StatementNode;
use super::super::node::{
    AstNode, ChoiceCompactArmKind, ChoiceEnabledFieldKind, ChoiceGotoActionKind,
    ChoiceHotkeyFieldKind, ChoiceIdFieldKind, ChoiceLabelFieldKind, ChoiceOptionBodyKind,
    ChoiceOptionForKind, ChoiceOptionKind, ChoiceOrderFieldKind, ChoiceOutActionKind,
    ChoiceSelectFieldKind, ChoiceValueFieldKind, ChoiceViewBodyKind, ChoiceViewFieldKind,
    ChoiceVisibleFieldKind, CloseBraceKind, ErrorNodeKind, MissingBodyKind, MissingExpressionKind,
    OpenBraceKind,
};
use super::super::source_file::AttachedDelimiterState;
use super::super::thread_body::AttachedRequiredNestedThreadFlowBody;
use super::{
    AttachedChoiceEntityReference, AttachedChoiceSuiteSource,
    AttachedRequiredChoiceEntityReference, pattern_has_recovery, required_expression_has_recovery,
    syntax_has_recovery,
};

/// Full Choice option with an expression/static ID and typed field body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceOption {
    pub(super) syntax: AstNode<ChoiceOptionKind>,
    pub(super) id: RequiredStatementExpressionNode,
    pub(super) body: AttachedRequiredChoiceOptionBody,
}

impl AttachedChoiceOption {
    pub const fn syntax(&self) -> &AstNode<ChoiceOptionKind> {
        &self.syntax
    }

    pub const fn id(&self) -> &RequiredStatementExpressionNode {
        &self.id
    }

    pub const fn body(&self) -> &AttachedRequiredChoiceOptionBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        required_expression_has_recovery(&self.id) || self.body.has_recovery()
    }
}

/// Pattern-driven Choice option sugar with a typed source expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceOptionFor {
    pub(super) syntax: AstNode<ChoiceOptionForKind>,
    pub(super) pattern: AttachedPatternNode,
    pub(super) source: RequiredStatementExpressionNode,
    pub(super) body: AttachedRequiredChoiceOptionBody,
}

impl AttachedChoiceOptionFor {
    pub const fn syntax(&self) -> &AstNode<ChoiceOptionForKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn source(&self) -> &RequiredStatementExpressionNode {
        &self.source
    }

    pub const fn body(&self) -> &AttachedRequiredChoiceOptionBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        pattern_has_recovery(&self.pattern)
            || required_expression_has_recovery(&self.source)
            || self.body.has_recovery()
    }
}

/// Present option field body or exact missing-body insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredChoiceOptionBody {
    Present(AttachedChoiceOptionBody),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedRequiredChoiceOptionBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Ordered typed fields of one full Choice option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceOptionBody {
    pub(super) syntax: AstNode<ChoiceOptionBodyKind>,
    pub(super) source: AttachedChoiceSuiteSource,
    pub(super) fields: Box<[AttachedChoiceOptionField]>,
    pub(super) recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedChoiceOptionBody {
    pub const fn syntax(&self) -> &AstNode<ChoiceOptionBodyKind> {
        &self.syntax
    }

    pub fn fields(&self) -> &[AttachedChoiceOptionField] {
        &self.fields
    }

    pub const fn source(&self) -> &AttachedChoiceSuiteSource {
        &self.source
    }

    pub fn recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.source.has_recovery()
            || !self.recovery.is_empty()
            || self
                .fields
                .iter()
                .any(AttachedChoiceOptionField::has_recovery)
    }
}

/// Closed full-option field family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedChoiceOptionField {
    Label {
        syntax: AstNode<ChoiceLabelFieldKind>,
        text_key: Option<AttachedRequiredChoiceEntityReference>,
        value: RequiredStatementExpressionNode,
    },
    Id {
        syntax: AstNode<ChoiceIdFieldKind>,
        value: RequiredStatementExpressionNode,
    },
    Value {
        syntax: AstNode<ChoiceValueFieldKind>,
        value: RequiredStatementExpressionNode,
    },
    Visible {
        syntax: AstNode<ChoiceVisibleFieldKind>,
        value: RequiredStatementExpressionNode,
    },
    Enabled {
        syntax: AstNode<ChoiceEnabledFieldKind>,
        value: RequiredStatementExpressionNode,
    },
    Order {
        syntax: AstNode<ChoiceOrderFieldKind>,
        value: RequiredStatementExpressionNode,
    },
    Hotkey {
        syntax: AstNode<ChoiceHotkeyFieldKind>,
        value: RequiredStatementExpressionNode,
    },
    View(AttachedChoiceView),
    Select(AttachedChoiceSelect),
    Let(StatementNode),
    Recovered(AstNode<ErrorNodeKind>),
}

impl AttachedChoiceOptionField {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Label {
                text_key, value, ..
            } => {
                text_key
                    .as_ref()
                    .is_some_and(AttachedRequiredChoiceEntityReference::has_recovery)
                    || required_expression_has_recovery(value)
            }
            Self::Id { value, .. }
            | Self::Value { value, .. }
            | Self::Visible { value, .. }
            | Self::Enabled { value, .. }
            | Self::Order { value, .. }
            | Self::Hotkey { value, .. } => required_expression_has_recovery(value),
            Self::View(value) => value.has_recovery(),
            Self::Select(value) => value.has_recovery(),
            Self::Let(statement) => syntax_has_recovery(&statement.syntax()),
            Self::Recovered(_) => true,
        }
    }
}

/// Typed option View projection with source-ordered named entries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceView {
    pub(super) syntax: AstNode<ChoiceViewFieldKind>,
    pub(super) body: AttachedRequiredChoiceViewBody,
}

impl AttachedChoiceView {
    pub const fn syntax(&self) -> &AstNode<ChoiceViewFieldKind> {
        &self.syntax
    }

    pub const fn body(&self) -> &AttachedRequiredChoiceViewBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.body.has_recovery()
    }
}

/// Present View field body or exact missing-body insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredChoiceViewBody {
    Present(AttachedChoiceViewBody),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedRequiredChoiceViewBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Delimited option View field body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceViewBody {
    pub(super) syntax: AstNode<ChoiceViewBodyKind>,
    pub(super) open: AstNode<OpenBraceKind>,
    pub(super) fields: Box<[AttachedChoiceViewEntry]>,
    pub(super) close: AstNode<CloseBraceKind>,
}

impl AttachedChoiceViewBody {
    pub const fn syntax(&self) -> &AstNode<ChoiceViewBodyKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub fn fields(&self) -> &[AttachedChoiceViewEntry] {
        &self.fields
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self
                .fields
                .iter()
                .any(AttachedChoiceViewEntry::has_recovery)
    }
}

/// One typed View field key/value relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceViewEntry {
    pub(super) syntax: AstNode<ChoiceViewFieldKind>,
    pub(super) key: RequiredStatementExpressionNode,
    pub(super) value: RequiredStatementExpressionNode,
}

impl AttachedChoiceViewEntry {
    pub const fn syntax(&self) -> &AstNode<ChoiceViewFieldKind> {
        &self.syntax
    }

    pub const fn key(&self) -> &RequiredStatementExpressionNode {
        &self.key
    }

    pub const fn value(&self) -> &RequiredStatementExpressionNode {
        &self.value
    }

    pub fn has_recovery(&self) -> bool {
        required_expression_has_recovery(&self.key) || required_expression_has_recovery(&self.value)
    }
}

/// Statement-only selection action owned by one option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceSelect {
    pub(super) syntax: AstNode<ChoiceSelectFieldKind>,
    pub(super) body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedChoiceSelect {
    pub const fn syntax(&self) -> &AstNode<ChoiceSelectFieldKind> {
        &self.syntax
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.body.has_recovery()
    }
}

/// Compact static option arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceCompactArm {
    pub(super) syntax: AstNode<ChoiceCompactArmKind>,
    pub(super) id: AttachedChoiceEntityReference,
    pub(super) label: RequiredStatementExpressionNode,
    pub(super) condition: Option<RequiredStatementExpressionNode>,
    pub(super) action: AttachedChoiceCompactAction,
}

impl AttachedChoiceCompactArm {
    pub const fn syntax(&self) -> &AstNode<ChoiceCompactArmKind> {
        &self.syntax
    }

    pub const fn id(&self) -> &AttachedChoiceEntityReference {
        &self.id
    }

    pub const fn label(&self) -> &RequiredStatementExpressionNode {
        &self.label
    }

    pub const fn condition(&self) -> Option<&RequiredStatementExpressionNode> {
        self.condition.as_ref()
    }

    pub const fn action(&self) -> &AttachedChoiceCompactAction {
        &self.action
    }

    pub fn has_recovery(&self) -> bool {
        self.id.has_recovery()
            || required_expression_has_recovery(&self.label)
            || self
                .condition
                .as_ref()
                .is_some_and(required_expression_has_recovery)
            || self.action.has_recovery()
    }
}

/// Typed compact action or exact missing-action insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedChoiceCompactAction {
    Goto {
        syntax: AstNode<ChoiceGotoActionKind>,
        target: AttachedRequiredChoiceEntityReference,
    },
    Out {
        syntax: AstNode<ChoiceOutActionKind>,
        value: RequiredStatementExpressionNode,
    },
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedChoiceCompactAction {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Goto { target, .. } => target.has_recovery(),
            Self::Out { value, .. } => required_expression_has_recovery(value),
            Self::Missing(_) => true,
        }
    }
}
