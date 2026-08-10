//! Exact-role child access over snapshot-bound attached syntax.

use super::family::{
    AstNodeFamily, BodyFamily, BodyNode, DelimiterFamily, DelimiterNode, ExprNode,
    ExpressionFamily, FamilyNode, FamilySpec, NameFamily, NameNode, PatternFamily, PatternNode,
    RecoveryFamily, RecoveryNode, RichTextFamily, RichTextNode, StatementFamily, StatementNode,
    TypeFamily, TypeNode,
};
use super::item::TypedItemNode;
use super::node::{
    AssertionStatementKind, AssignmentStatementKind, AstKind, AstNode, BinaryExpressionKind,
    BlockKind, CallArgumentKind, CallExpressionKind, ChoiceExpressionKind, CloseParenKind,
    CloseStatementKind, DeclarationHeaderKind, DeclarationPublicIdKind, DocBlockKind, ExactAstKind,
    ExpressionBodyKind, ExpressionStatementKind, FixedParameterGroupKind, FunctionBodyKind,
    FunctionTypeKind, GenericApplicationTypeKind, IfStatementKind, LetChoiceStatementKind,
    LetStatementKind, LifetimeSetStatementKind, MatchArmKind, MatchStatementKind, MissingBodyKind,
    MissingExpressionKind, NameReferenceKind, OmittedBlockTailKind, OpenParenKind,
    OuterAttributeKind, ParameterKind, PredicateBlockKind, PredicateBodyKind, ProofBlockKind,
    ProofBodyKind, ProofCallStatementKind, RecordPatternFieldKind, RecordPatternKind,
    ReturnStatementKind, RichTextArgumentPayloadKind, RichTextArgumentTokenKind,
    RichTextArgumentValueKind, RichTextConditionPayloadKind, RichTextDialogueCallPayloadKind,
    RichTextEndTagKind, RichTextFxCallPayloadKind, RichTextInvalidArgumentKind,
    RichTextNamedArgumentKind, RichTextPositionalArgumentKind, RichTextTagKind,
    RichTextTagNameKind, SelectStatementKind, SourceItemKind, TypeArgumentKind,
    UnsafeLifetimeStatementKind, VisibilityKind, WaitStatementKind, WholeBindingPatternKind,
    YieldStatementKind,
};
use super::{SyntaxAccessError, SyntaxNodeHandle};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

impl SyntaxNodeHandle {
    pub(crate) fn optional_unique_child(
        &self,
        role: SyntaxRole,
    ) -> Result<Option<Self>, SyntaxAccessError> {
        let children = self.children_with_role(role);
        match children.as_slice() {
            [] => Ok(None),
            [child] => Ok(Some(child.clone())),
            _ => Err(SyntaxAccessError::AmbiguousChild {
                parent: self.id(),
                role,
                count: children.len(),
            }),
        }
    }

    pub(super) fn ordered_children(
        &self,
        role: SyntaxRoleClass,
    ) -> Result<Vec<Self>, SyntaxAccessError> {
        if !role_class_is_ordinal(role) {
            return Err(SyntaxAccessError::NonOrdinalRoleClass { role });
        }
        let children = self
            .children()
            .into_iter()
            .filter(|child| child.role().class() == role)
            .collect::<Vec<_>>();
        for (expected, child) in children.iter().enumerate() {
            let expected = u32::try_from(expected).unwrap_or(u32::MAX);
            let actual = child
                .role()
                .ordinal()
                .expect("ordinal role class has an ordinal");
            if actual != expected {
                return Err(SyntaxAccessError::NonContiguousRole {
                    parent: self.id(),
                    role,
                    expected,
                    actual,
                });
            }
        }
        Ok(children)
    }
}

const fn role_class_is_ordinal(role: SyntaxRoleClass) -> bool {
    matches!(
        role,
        SyntaxRoleClass::Attribute
            | SyntaxRoleClass::GenericParameter
            | SyntaxRoleClass::Parameter
            | SyntaxRoleClass::WherePredicate
            | SyntaxRoleClass::ContractClause
            | SyntaxRoleClass::ContractOperand
            | SyntaxRoleClass::Statement
            | SyntaxRoleClass::ThreadFlowItem
            | SyntaxRoleClass::ChoiceItem
            | SyntaxRoleClass::ChoicePlanItem
            | SyntaxRoleClass::ChoiceOptionField
            | SyntaxRoleClass::ChoiceViewField
            | SyntaxRoleClass::Branch
            | SyntaxRoleClass::TrailingRecovery
            | SyntaxRoleClass::Argument
            | SyntaxRoleClass::RichTextTag
            | SyntaxRoleClass::MatchArm
            | SyntaxRoleClass::Field
            | SyntaxRoleClass::Member
            | SyntaxRoleClass::InputPort
            | SyntaxRoleClass::OutputPort
            | SyntaxRoleClass::Export
            | SyntaxRoleClass::Label
            | SyntaxRoleClass::Bucket
            | SyntaxRoleClass::Policy
            | SyntaxRoleClass::Reference
            | SyntaxRoleClass::RelatedReference
            | SyntaxRoleClass::Element
            | SyntaxRoleClass::Recovery
    )
}

impl<K: AstKind> AstNode<K> {
    pub(crate) fn required_exact_child<C: ExactAstKind>(
        &self,
        role: SyntaxRole,
    ) -> Result<AstNode<C>, SyntaxAccessError> {
        let syntax = self.syntax().optional_unique_child(role)?.ok_or(
            SyntaxAccessError::MissingExactChild {
                parent: self.id(),
                role,
                expected: C::KIND,
            },
        )?;
        Ok(syntax.cast()?)
    }

    pub(crate) fn optional_exact_child<C: ExactAstKind>(
        &self,
        role: SyntaxRole,
    ) -> Result<Option<AstNode<C>>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(role)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub(crate) fn required_family_child<F: FamilySpec>(
        &self,
        role: SyntaxRole,
    ) -> Result<FamilyNode<F>, SyntaxAccessError> {
        let syntax = self.syntax().optional_unique_child(role)?.ok_or(
            SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role,
                expected: F::FAMILY,
            },
        )?;
        FamilyNode::new(syntax)
    }

    pub(crate) fn optional_family_child<F: FamilySpec>(
        &self,
        role: SyntaxRole,
    ) -> Result<Option<FamilyNode<F>>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(role)?
            .map(FamilyNode::new)
            .transpose()
    }

    pub(crate) fn family_children<F: FamilySpec>(
        &self,
        role: SyntaxRole,
    ) -> Result<Vec<FamilyNode<F>>, SyntaxAccessError> {
        self.syntax()
            .children_with_role(role)
            .into_iter()
            .map(FamilyNode::new)
            .collect()
    }

    pub(crate) fn ordered_exact_children<C: ExactAstKind>(
        &self,
        role: SyntaxRoleClass,
    ) -> Result<Vec<AstNode<C>>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(role)?
            .into_iter()
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .collect()
    }

    pub(crate) fn ordered_family_children<F: FamilySpec>(
        &self,
        role: SyntaxRoleClass,
    ) -> Result<Vec<FamilyNode<F>>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(role)?
            .into_iter()
            .map(FamilyNode::new)
            .collect()
    }
}

impl AstNode<SourceItemKind> {
    pub fn header(&self) -> Result<AstNode<DeclarationHeaderKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::Element(0))
    }

    pub fn public_id(&self) -> Result<Option<AstNode<DeclarationPublicIdKind>>, SyntaxAccessError> {
        self.header()?.optional_exact_child(SyntaxRole::PublicId)
    }

    pub fn name(&self) -> Result<Option<NameNode>, SyntaxAccessError> {
        self.header()?
            .optional_family_child::<NameFamily>(SyntaxRole::Name)
    }

    pub fn source_type(&self) -> Result<Option<TypeNode>, SyntaxAccessError> {
        self.header()?
            .optional_family_child::<TypeFamily>(SyntaxRole::Type)
    }

    pub fn body(&self) -> Result<DeclarationBodyNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Body,
            })?;
        DeclarationBodyNode::new(syntax)
    }
}

impl AstNode<ExpressionStatementKind> {
    pub fn source_initializer(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Initializer)
    }
}

/// Authored declaration body or an exact missing-body recovery node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationBodyNode {
    Body(BodyNode),
    Missing(AstNode<MissingBodyKind>),
}

impl DeclarationBodyNode {
    fn new(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        match syntax.kind() {
            SyntaxKind::MissingBody => Ok(Self::Missing(syntax.cast()?)),
            _ => Ok(Self::Body(FamilyNode::<BodyFamily>::new(syntax)?)),
        }
    }

    pub fn range(&self) -> arcweft_source::SourceRange {
        match self {
            Self::Body(body) => body.range(),
            Self::Missing(body) => body.range(),
        }
    }
}

impl TypedItemNode {
    fn prefix_owner(&self) -> Result<SyntaxNodeHandle, SyntaxAccessError> {
        let owner = self.syntax();
        let headers = owner
            .children()
            .into_iter()
            .filter(|child| child.kind() == SyntaxKind::DeclarationHeader)
            .collect::<Vec<_>>();
        match headers.as_slice() {
            [] => Ok(owner),
            [header] => Ok(header.clone()),
            _ => Err(SyntaxAccessError::AmbiguousChild {
                parent: owner.id(),
                role: SyntaxRole::Element(0),
                count: headers.len(),
            }),
        }
    }

    pub fn declaration_header(
        &self,
    ) -> Result<Option<AstNode<DeclarationHeaderKind>>, SyntaxAccessError> {
        let Some(candidate) = self
            .syntax()
            .optional_unique_child(SyntaxRole::Element(0))?
        else {
            return Ok(None);
        };
        if candidate.kind() != SyntaxKind::DeclarationHeader {
            return Ok(None);
        }
        Ok(Some(candidate.cast()?))
    }

    pub fn documentation(&self) -> Result<Option<AstNode<DocBlockKind>>, SyntaxAccessError> {
        self.prefix_owner()?
            .optional_unique_child(SyntaxRole::Documentation)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub fn attributes(&self) -> Result<Vec<AstNode<OuterAttributeKind>>, SyntaxAccessError> {
        self.prefix_owner()?
            .ordered_children(SyntaxRoleClass::Attribute)?
            .into_iter()
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .collect()
    }

    pub fn visibility(&self) -> Result<Option<AstNode<VisibilityKind>>, SyntaxAccessError> {
        self.prefix_owner()?
            .optional_unique_child(SyntaxRole::Visibility)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub fn name(&self) -> Result<Option<NameNode>, SyntaxAccessError> {
        self.prefix_owner()?
            .optional_unique_child(SyntaxRole::Name)?
            .map(FamilyNode::<NameFamily>::new)
            .transpose()
    }

    pub fn body(&self) -> Result<Option<DeclarationBodyNode>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .map(DeclarationBodyNode::new)
            .transpose()
    }

    pub fn recovery(&self) -> Result<Vec<RecoveryNode>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(SyntaxRoleClass::Recovery)?
            .into_iter()
            .map(FamilyNode::<RecoveryFamily>::new)
            .collect()
    }
}

impl AstNode<DeclarationHeaderKind> {
    pub fn documentation(&self) -> Result<Option<AstNode<DocBlockKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Documentation)
    }

    pub fn attributes(&self) -> Result<Vec<AstNode<OuterAttributeKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Attribute)
    }

    pub fn visibility(&self) -> Result<Option<AstNode<VisibilityKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Visibility)
    }

    pub fn name(&self) -> Result<Option<NameNode>, SyntaxAccessError> {
        self.optional_family_child::<NameFamily>(SyntaxRole::Name)
    }
}

impl AstNode<PredicateBodyKind> {
    pub fn content(&self) -> Result<DeclarationBodyNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Body,
            })?;
        DeclarationBodyNode::new(syntax)
    }
}

impl AstNode<ProofBodyKind> {
    pub fn content(&self) -> Result<DeclarationBodyNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Body,
            })?;
        DeclarationBodyNode::new(syntax)
    }
}

impl AstNode<FunctionBodyKind> {
    pub fn content(&self) -> Result<DeclarationBodyNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Body,
            })?;
        DeclarationBodyNode::new(syntax)
    }
}

impl AstNode<ExpressionBodyKind> {
    pub fn expression(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Body)
    }
}

/// Typed head of a statement-form conditional.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IfStatementHeadNode {
    /// Ordinary `if condition` head.
    Condition(ExprNode),
    /// Pattern-matching `if let pattern = scrutinee [when guard]` head.
    Let {
        pattern: PatternNode,
        scrutinee: ExprNode,
        guard: Option<ExprNode>,
    },
}

/// Typed authored branch following an `else` keyword.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IfStatementElseNode {
    /// Braced terminal else branch.
    Block(AstNode<BlockKind>),
    /// Nested `else if` or `else if let` statement.
    If(StatementNode),
}

/// Authored block tail or the exact zero-width omitted-tail marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockTailNode {
    Expression(ExprNode),
    Omitted(AstNode<OmittedBlockTailKind>),
}

/// Authored or exactly missing initializer of an ordinary `let` statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LetInitializerNode {
    /// Source-backed initializer expression.
    Expression(ExprNode),
    /// Exact zero-width recovery insertion after the authored initializer head.
    Missing(AstNode<MissingExpressionKind>),
}

/// Authored or exactly missing required expression owned by a statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequiredStatementExpressionNode {
    /// Source-backed operand expression.
    Expression(ExprNode),
    /// Exact zero-width recovery insertion for an omitted required operand.
    Missing(AstNode<MissingExpressionKind>),
}

/// Typed unsafe-audit identity or its exact missing recovery slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsafeAuditIdNode {
    /// Source-backed entity-reference expression selected by the parser.
    Reference(ExprNode),
    /// Exact zero-width recovery insertion for an omitted audit identity.
    Missing(AstNode<MissingExpressionKind>),
}

/// Optional unsafe-audit reason value, retaining a present missing slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsafeAuditReasonNode {
    /// Source-backed reason expression.
    Expression(ExprNode),
    /// Exact zero-width recovery insertion after an authored `reason` head.
    Missing(AstNode<MissingExpressionKind>),
}

/// Braced unsafe-audit body or its exact missing recovery owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsafeAuditBodyNode {
    /// Source-backed statement block.
    Block(AstNode<BlockKind>),
    /// Exact zero-width recovery node for a missing braced body.
    Missing(AstNode<MissingBodyKind>),
}

/// Exact authored or recovered body retained by one statement-form Match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchStatementArmBodyNode {
    /// Source-backed expression body.
    Expression(ExprNode),
    /// Source-backed statement-only block body.
    Block(AstNode<BlockKind>),
    /// Exact zero-width recovery insertion for a missing required body.
    Missing(AstNode<MissingExpressionKind>),
}

/// Authored expression or exact required-expression recovery in statement Match syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchStatementExpressionNode {
    Expression(ExprNode),
    Missing(AstNode<MissingExpressionKind>),
}

/// Source-backed braced body or exact missing-body recovery for statement Match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchStatementBodyNode {
    Block(AstNode<BlockKind>),
    Missing(AstNode<MissingBodyKind>),
}

impl MatchStatementBodyNode {
    /// Source-ordered arms retained by an authored body. Missing bodies own no arms.
    pub fn arms(&self) -> Result<Vec<AstNode<MatchArmKind>>, SyntaxAccessError> {
        let Self::Block(block) = self else {
            return Ok(Vec::new());
        };
        block
            .syntax()
            .ordered_children(SyntaxRoleClass::MatchArm)?
            .into_iter()
            .map(|arm| arm.cast().map_err(SyntaxAccessError::from))
            .collect()
    }
}

impl BlockTailNode {
    fn new(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        match syntax.kind() {
            SyntaxKind::OmittedBlockTail => Ok(Self::Omitted(syntax.cast()?)),
            _ => Ok(Self::Expression(FamilyNode::<ExpressionFamily>::new(
                syntax,
            )?)),
        }
    }

    pub fn range(&self) -> arcweft_source::SourceRange {
        match self {
            Self::Expression(expression) => expression.range(),
            Self::Omitted(omitted) => omitted.range(),
        }
    }

    pub fn source_span(&self) -> arcweft_source::SourceSpan {
        match self {
            Self::Expression(expression) => expression.source_span(),
            Self::Omitted(omitted) => omitted.source_span(),
        }
    }
}

macro_rules! impl_block_access {
    ($kind:ty) => {
        impl AstNode<$kind> {
            pub fn open_delimiter(&self) -> Result<DelimiterNode, SyntaxAccessError> {
                self.required_family_child::<DelimiterFamily>(SyntaxRole::OpenDelimiter)
            }

            pub fn statements(&self) -> Result<Vec<StatementNode>, SyntaxAccessError> {
                self.ordered_family_children::<StatementFamily>(SyntaxRoleClass::Statement)
            }

            /// Returns the value tail when this is a value block. Statement
            /// blocks intentionally return `None` and own statements only.
            pub fn optional_tail(&self) -> Result<Option<BlockTailNode>, SyntaxAccessError> {
                self.syntax()
                    .optional_unique_child(SyntaxRole::Tail)?
                    .map(BlockTailNode::new)
                    .transpose()
            }

            pub fn tail(&self) -> Result<BlockTailNode, SyntaxAccessError> {
                self.optional_tail()?
                    .ok_or(SyntaxAccessError::MissingFamilyChild {
                        parent: self.id(),
                        role: SyntaxRole::Tail,
                        expected: AstNodeFamily::Expression,
                    })
            }

            pub fn close_delimiter(&self) -> Result<DelimiterNode, SyntaxAccessError> {
                self.required_family_child::<DelimiterFamily>(SyntaxRole::CloseDelimiter)
            }
        }
    };
}

impl_block_access!(PredicateBlockKind);
impl_block_access!(ProofBlockKind);
impl_block_access!(BlockKind);

impl AstNode<FunctionBodyKind> {
    /// Exact braced computation block owned by an ordinary function body.
    pub fn block(&self) -> Result<AstNode<BlockKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::Body)
    }
}

impl AstNode<AssertionStatementKind> {
    pub fn conditions(&self) -> Result<Vec<ExprNode>, SyntaxAccessError> {
        self.family_children::<ExpressionFamily>(SyntaxRole::Condition)
    }
}

impl AstNode<AssignmentStatementKind> {
    /// Exact typed assignment target selected by the statement grammar.
    pub fn target(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Target)
    }

    /// Exact typed assigned value, including a required missing-expression slot.
    pub fn value(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Initializer)
    }
}

impl AstNode<LifetimeSetStatementKind> {
    /// Exact typed lifetime-registry target selected by the statement grammar.
    pub fn target(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Target)
    }

    /// Exact typed lifetime value, including a required missing-expression slot.
    pub fn value(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Initializer)
    }
}

impl AstNode<ReturnStatementKind> {
    /// Exact returned value, including the required missing-expression slot.
    pub fn value(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Operand)
    }
}

impl AstNode<YieldStatementKind> {
    /// Exact yielded value, including the required missing-expression slot.
    pub fn expression(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Operand)
    }
}

impl AstNode<WaitStatementKind> {
    /// Canonical opening parenthesis or its exact parser-selected insertion.
    pub fn open_delimiter(&self) -> Result<AstNode<OpenParenKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::OpenDelimiter)
    }

    /// Exact wait target, including the required missing-expression slot.
    pub fn target(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Operand)
    }

    /// Canonical closing parenthesis or its exact parser-selected insertion.
    pub fn close_delimiter(&self) -> Result<AstNode<CloseParenKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::CloseDelimiter)
    }

    /// Whether current-grammar recovery inserted either required parenthesis.
    pub fn has_punctuation_recovery(&self) -> Result<bool, SyntaxAccessError> {
        Ok(self.open_delimiter()?.range().is_empty() || self.close_delimiter()?.range().is_empty())
    }
}

impl AstNode<CloseStatementKind> {
    /// Exact resource target, including the required missing-expression slot.
    pub fn target(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        required_statement_expression(self, SyntaxRole::Operand)
    }
}

impl AstNode<SelectStatementKind> {
    /// Exact ordinary Select operand.
    ///
    /// A block-family operand belongs to the separately designed Flow/Thread
    /// surface and is deliberately not admitted through this statement view.
    pub fn expression(&self) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
        let expression = required_statement_expression(self, SyntaxRole::Operand)?;
        if let RequiredStatementExpressionNode::Expression(authored) = &expression {
            let semantic = authored.semantic()?;
            if semantic.projection().is_value_block() {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: authored.id() });
            }
        }
        Ok(expression)
    }
}

pub(super) fn required_statement_expression<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
    let syntax = owner.syntax().optional_unique_child(role)?.ok_or(
        SyntaxAccessError::MissingFamilyChild {
            parent: owner.id(),
            role,
            expected: AstNodeFamily::Expression,
        },
    )?;
    if syntax.kind() == SyntaxKind::MissingExpression {
        Ok(RequiredStatementExpressionNode::Missing(syntax.cast()?))
    } else {
        Ok(RequiredStatementExpressionNode::Expression(FamilyNode::<
            ExpressionFamily,
        >::new(
            syntax
        )?))
    }
}

impl AstNode<UnsafeLifetimeStatementKind> {
    /// Typed audit identity retained without reading or splitting source text.
    pub fn audit_id(&self) -> Result<UnsafeAuditIdNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Reference(0))?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Reference(0),
                expected: AstNodeFamily::Expression,
            })?;
        match syntax.kind() {
            SyntaxKind::EntityReferenceExpression => Ok(UnsafeAuditIdNode::Reference(
                FamilyNode::<ExpressionFamily>::new(syntax)?,
            )),
            SyntaxKind::MissingExpression => Ok(UnsafeAuditIdNode::Missing(syntax.cast()?)),
            actual_kind => Err(SyntaxAccessError::FamilyMismatch {
                id: syntax.id(),
                expected: AstNodeFamily::Expression,
                actual_kind,
                actual_tag: syntax.tag(),
            }),
        }
    }

    /// Optional authored reason. A present `reason` head with no value keeps a
    /// typed missing slot instead of becoming indistinguishable from omission.
    pub fn reason(&self) -> Result<Option<UnsafeAuditReasonNode>, SyntaxAccessError> {
        let Some(syntax) = self
            .syntax()
            .optional_unique_child(SyntaxRole::Initializer)?
        else {
            return Ok(None);
        };
        Ok(Some(if syntax.kind() == SyntaxKind::MissingExpression {
            UnsafeAuditReasonNode::Missing(syntax.cast()?)
        } else {
            UnsafeAuditReasonNode::Expression(FamilyNode::<ExpressionFamily>::new(syntax)?)
        }))
    }

    /// Source-owned unsafe-lifetime body containing the audit insertion anchor.
    pub fn body(&self) -> Result<AstNode<BlockKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::Body)
    }

    /// Required body slot, preserving a recognized missing-body recovery.
    pub fn body_or_missing(&self) -> Result<UnsafeAuditBodyNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Body,
            })?;
        match syntax.kind() {
            SyntaxKind::Block => Ok(UnsafeAuditBodyNode::Block(syntax.cast()?)),
            SyntaxKind::MissingBody => Ok(UnsafeAuditBodyNode::Missing(syntax.cast()?)),
            actual_kind => Err(SyntaxAccessError::FamilyMismatch {
                id: syntax.id(),
                expected: AstNodeFamily::Body,
                actual_kind,
                actual_tag: syntax.tag(),
            }),
        }
    }

    /// Exact SAFETY documentation nodes recognized by the grammar transaction.
    pub fn safety_documentation(&self) -> Result<Vec<AstNode<DocBlockKind>>, SyntaxAccessError> {
        self.body()?
            .syntax()
            .children_with_role(SyntaxRole::Documentation)
            .into_iter()
            .map(|documentation| documentation.cast().map_err(SyntaxAccessError::from))
            .collect()
    }

    /// Exact authored opening delimiter used for an audit insertion edit.
    ///
    /// Consumers retain this delimiter's qualified syntax identity in the HIR
    /// source-component table; they do not copy its offset into semantic HIR.
    pub fn audit_insertion_anchor(&self) -> Result<DelimiterNode, SyntaxAccessError> {
        self.body()?.open_delimiter()
    }
}

impl AstNode<IfStatementKind> {
    /// Exact typed conditional head without reparsing source text.
    pub fn head(&self) -> Result<IfStatementHeadNode, SyntaxAccessError> {
        if let Some(pattern) = self.optional_family_child::<PatternFamily>(SyntaxRole::Pattern)? {
            return Ok(IfStatementHeadNode::Let {
                pattern,
                scrutinee: self.required_family_child::<ExpressionFamily>(SyntaxRole::Scrutinee)?,
                guard: self.optional_family_child::<ExpressionFamily>(SyntaxRole::Guard)?,
            });
        }
        Ok(IfStatementHeadNode::Condition(
            self.required_family_child::<ExpressionFamily>(SyntaxRole::Condition)?,
        ))
    }

    /// Required braced then branch.
    pub fn then_branch(&self) -> Result<AstNode<BlockKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::ThenBranch)
    }

    /// Optional typed else block or nested conditional statement.
    pub fn else_branch(&self) -> Result<Option<IfStatementElseNode>, SyntaxAccessError> {
        let Some(branch) = self
            .syntax()
            .optional_unique_child(SyntaxRole::ElseBranch)?
        else {
            return Ok(None);
        };
        Ok(Some(match branch.kind() {
            SyntaxKind::Block => IfStatementElseNode::Block(branch.cast()?),
            SyntaxKind::IfStatement => {
                IfStatementElseNode::If(FamilyNode::<StatementFamily>::new(branch)?)
            }
            actual_kind => {
                return Err(SyntaxAccessError::FamilyMismatch {
                    id: branch.id(),
                    expected: AstNodeFamily::Statement,
                    actual_kind,
                    actual_tag: branch.tag(),
                });
            }
        }))
    }
}

impl AstNode<MatchStatementKind> {
    /// Exact source-backed scrutinee of this statement-form Match.
    pub fn scrutinee(&self) -> Result<MatchStatementExpressionNode, SyntaxAccessError> {
        match_statement_expression(self, SyntaxRole::Scrutinee)
    }

    /// Required braced Match body or its exact missing recovery owner.
    pub fn body_or_missing(&self) -> Result<MatchStatementBodyNode, SyntaxAccessError> {
        let body = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Body,
            })?;
        match body.kind() {
            SyntaxKind::Block => Ok(MatchStatementBodyNode::Block(body.cast()?)),
            SyntaxKind::MissingBody => Ok(MatchStatementBodyNode::Missing(body.cast()?)),
            actual_kind => Err(SyntaxAccessError::FamilyMismatch {
                id: body.id(),
                expected: AstNodeFamily::Body,
                actual_kind,
                actual_tag: body.tag(),
            }),
        }
    }
}

impl AstNode<MatchArmKind> {
    pub fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }

    pub fn guard(&self) -> Result<Option<MatchStatementExpressionNode>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(SyntaxRole::Guard)?
            .map(match_statement_expression_from_syntax)
            .transpose()
    }

    pub fn body(&self) -> Result<MatchStatementArmBodyNode, SyntaxAccessError> {
        let syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role: SyntaxRole::Body,
                expected: AstNodeFamily::Expression,
            })?;
        match syntax.kind() {
            SyntaxKind::Block => Ok(MatchStatementArmBodyNode::Block(syntax.cast()?)),
            SyntaxKind::MissingExpression => Ok(MatchStatementArmBodyNode::Missing(syntax.cast()?)),
            _ => Ok(MatchStatementArmBodyNode::Expression(FamilyNode::<
                ExpressionFamily,
            >::new(
                syntax
            )?)),
        }
    }
}

fn match_statement_expression<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<MatchStatementExpressionNode, SyntaxAccessError> {
    let syntax = owner.syntax().optional_unique_child(role)?.ok_or(
        SyntaxAccessError::MissingFamilyChild {
            parent: owner.id(),
            role,
            expected: AstNodeFamily::Expression,
        },
    )?;
    match_statement_expression_from_syntax(syntax)
}

fn match_statement_expression_from_syntax(
    syntax: SyntaxNodeHandle,
) -> Result<MatchStatementExpressionNode, SyntaxAccessError> {
    if syntax.kind() == SyntaxKind::MissingExpression {
        Ok(MatchStatementExpressionNode::Missing(syntax.cast()?))
    } else {
        Ok(MatchStatementExpressionNode::Expression(FamilyNode::<
            ExpressionFamily,
        >::new(
            syntax
        )?))
    }
}

impl AstNode<LetStatementKind> {
    pub fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }

    pub fn initializer(&self) -> Result<Option<LetInitializerNode>, SyntaxAccessError> {
        let Some(syntax) = self
            .syntax()
            .optional_unique_child(SyntaxRole::Initializer)?
        else {
            return Ok(None);
        };
        Ok(Some(if syntax.kind() == SyntaxKind::MissingExpression {
            LetInitializerNode::Missing(syntax.cast()?)
        } else {
            LetInitializerNode::Expression(FamilyNode::<ExpressionFamily>::new(syntax)?)
        }))
    }
}

impl AstNode<LetChoiceStatementKind> {
    /// Binding pattern published only after the complete Choice initializer.
    pub fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }

    /// Exact shared Choice expression owner used by direct and binding forms.
    pub fn initializer(&self) -> Result<AstNode<ChoiceExpressionKind>, SyntaxAccessError> {
        self.required_exact_child::<ChoiceExpressionKind>(SyntaxRole::Initializer)
    }
}

impl AstNode<ExpressionStatementKind> {
    /// Exact value expression evaluated by this ordinary expression statement.
    pub fn expression(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Initializer)
    }
}

impl AstNode<ProofCallStatementKind> {
    pub fn callee(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Callee)
    }
}

impl AstNode<CallExpressionKind> {
    pub fn callee(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Callee)
    }

    pub fn arguments(&self) -> Result<Vec<AstNode<CallArgumentKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Argument)
    }
}

impl AstNode<CallArgumentKind> {
    pub fn name(&self) -> Result<Option<AstNode<NameReferenceKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Name)
    }

    pub fn operand(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Operand)
    }
}

impl AstNode<RichTextTagKind> {
    pub fn name(&self) -> Result<AstNode<RichTextTagNameKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::Name)
    }

    pub fn payload(&self) -> Result<Option<RichTextNode>, SyntaxAccessError> {
        self.optional_family_child::<RichTextFamily>(SyntaxRole::Payload)
    }
}

impl AstNode<RichTextEndTagKind> {
    pub fn name(&self) -> Result<Option<AstNode<RichTextTagNameKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Name)
    }
}

impl AstNode<RichTextArgumentPayloadKind> {
    pub fn arguments(&self) -> Result<Vec<RichTextNode>, SyntaxAccessError> {
        self.ordered_family_children::<RichTextFamily>(SyntaxRoleClass::Argument)
    }
}

impl AstNode<RichTextNamedArgumentKind> {
    pub fn key(&self) -> Result<RichTextNode, SyntaxAccessError> {
        self.required_family_child::<RichTextFamily>(SyntaxRole::Key)
    }

    pub fn equals(&self) -> Result<RichTextNode, SyntaxAccessError> {
        self.required_family_child::<RichTextFamily>(SyntaxRole::Equals)
    }

    pub fn value(&self) -> Result<RichTextNode, SyntaxAccessError> {
        self.required_family_child::<RichTextFamily>(SyntaxRole::Value)
    }
}

impl AstNode<RichTextPositionalArgumentKind> {
    pub fn value(&self) -> Result<RichTextNode, SyntaxAccessError> {
        self.required_family_child::<RichTextFamily>(SyntaxRole::Value)
    }
}

impl AstNode<RichTextInvalidArgumentKind> {
    pub fn issue(&self) -> Result<RichTextNode, SyntaxAccessError> {
        self.required_family_child::<RichTextFamily>(SyntaxRole::Issue)
    }
}

impl AstNode<RichTextArgumentValueKind> {
    pub fn token(&self) -> Result<AstNode<RichTextArgumentTokenKind>, SyntaxAccessError> {
        self.required_exact_child(SyntaxRole::Token)
    }
}

impl AstNode<RichTextArgumentTokenKind> {
    pub fn content(&self) -> Result<RichTextNode, SyntaxAccessError> {
        self.required_family_child::<RichTextFamily>(SyntaxRole::Content)
    }

    pub fn opening_quote(&self) -> Result<Option<RichTextNode>, SyntaxAccessError> {
        self.optional_family_child::<RichTextFamily>(SyntaxRole::OpeningQuote)
    }

    pub fn closing_quote(&self) -> Result<Option<RichTextNode>, SyntaxAccessError> {
        self.optional_family_child::<RichTextFamily>(SyntaxRole::ClosingQuote)
    }
}

impl AstNode<RichTextFxCallPayloadKind> {
    pub fn expression(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Operand)
    }
}

impl AstNode<RichTextDialogueCallPayloadKind> {
    pub fn expression(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Operand)
    }
}

impl AstNode<RichTextConditionPayloadKind> {
    pub fn expression(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Condition)
    }
}

impl AstNode<BinaryExpressionKind> {
    pub fn left(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::LeftOperand)
    }

    pub fn right(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::RightOperand)
    }
}

impl AstNode<ParameterKind> {
    pub fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::ParameterPattern)
    }

    pub fn ty(&self) -> Result<TypeNode, SyntaxAccessError> {
        self.required_family_child::<TypeFamily>(SyntaxRole::ParameterType)
    }
}

impl AstNode<FixedParameterGroupKind> {
    pub fn parameters(&self) -> Result<Vec<AstNode<ParameterKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Parameter)
    }
}

impl AstNode<WholeBindingPatternKind> {
    pub fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }
}

impl AstNode<RecordPatternKind> {
    pub fn fields(&self) -> Result<Vec<PatternNode>, SyntaxAccessError> {
        self.ordered_family_children::<PatternFamily>(SyntaxRoleClass::Field)
    }
}

impl AstNode<RecordPatternFieldKind> {
    pub fn pattern(&self) -> Result<Option<PatternNode>, SyntaxAccessError> {
        self.optional_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }
}

impl AstNode<GenericApplicationTypeKind> {
    pub fn arguments(&self) -> Result<Vec<AstNode<TypeArgumentKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Argument)
    }
}

impl AstNode<TypeArgumentKind> {
    pub fn ty(&self) -> Result<TypeNode, SyntaxAccessError> {
        self.required_family_child::<TypeFamily>(SyntaxRole::Type)
    }
}

impl AstNode<FunctionTypeKind> {
    pub fn parameters(&self) -> Result<Vec<TypeNode>, SyntaxAccessError> {
        self.ordered_family_children::<TypeFamily>(SyntaxRoleClass::Element)
    }

    pub fn result(&self) -> Result<TypeNode, SyntaxAccessError> {
        self.required_family_child::<TypeFamily>(SyntaxRole::RightOperand)
    }
}

#[cfg(test)]
mod tests {
    use super::DeclarationBodyNode;
    use crate::attachment::family::{
        AttributeNode, BodyNode, DeclarationPartNode, DelimiterNode, PathNode,
    };

    #[test]
    fn private_family_aliases_remain_nameable_for_atomic_consumer_migration() {
        fn consume(
            _attribute: Option<AttributeNode>,
            _body: Option<BodyNode>,
            _part: Option<DeclarationPartNode>,
            _delimiter: Option<DelimiterNode>,
            _path: Option<PathNode>,
            _body_union: Option<DeclarationBodyNode>,
        ) {
        }
        consume(None, None, None, None, None, None);
    }
}
