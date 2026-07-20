//! Exact-role child access over snapshot-bound attached syntax.

#![allow(
    dead_code,
    reason = "the private accessor surface is consumed only after the atomic ParsedSource syntax switch"
)]

use std::sync::Arc;

use super::family::{
    AstNodeFamily, BodyFamily, BodyNode, DelimiterFamily, DelimiterNode, ExprNode,
    ExpressionFamily, FamilyNode, FamilySpec, ItemFamily, ItemNode, NameFamily, NameNode,
    PatternFamily, PatternNode, RecoveryFamily, RecoveryNode, StatementFamily, StatementNode,
    TypeFamily, TypeNode,
};
use super::node::{
    AssertionStatementKind, AstKind, AstNode, BinaryExpressionKind, CallArgumentKind,
    CallExpressionKind, DeclarationHeaderKind, DialogueCallExpressionKind, DocBlockKind,
    ExpressionBodyKind, FixedParameterGroupKind, FunctionTypeKind, GenericApplicationTypeKind,
    LetStatementKind, MissingBodyKind, NameReferenceKind, OmittedBlockTailKind, OuterAttributeKind,
    ParameterKind, PredicateBlockKind, PredicateBodyKind, ProofBlockKind, ProofBodyKind,
    ProofCallStatementKind, RecordPatternFieldKind, RecordPatternKind, SourceFileKind,
    TypeArgumentKind, VisibilityKind, WholeBindingPatternKind,
};
use super::{SyntaxAccessError, SyntaxLookupError, SyntaxNodeHandle, SyntaxSnapshotData};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

/// Private whole-source reader bound to one accepted immutable snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedSyntaxTree {
    root: AstNode<SourceFileKind>,
}

impl TypedSyntaxTree {
    pub(crate) const fn root(&self) -> &AstNode<SourceFileKind> {
        &self.root
    }

    pub(crate) fn items(&self) -> Result<Vec<ItemNode>, SyntaxAccessError> {
        self.root.items()
    }
}

impl SyntaxSnapshotData {
    pub(crate) fn typed_tree(self: &Arc<Self>) -> Result<TypedSyntaxTree, SyntaxLookupError> {
        Ok(TypedSyntaxTree {
            root: self.root_handle().cast()?,
        })
    }
}

impl SyntaxNodeHandle {
    fn optional_unique_child(&self, role: SyntaxRole) -> Result<Option<Self>, SyntaxAccessError> {
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

    fn ordered_children(&self, role: SyntaxRoleClass) -> Result<Vec<Self>, SyntaxAccessError> {
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
            | SyntaxRoleClass::RequiresClause
            | SyntaxRoleClass::EnsuresClause
            | SyntaxRoleClass::Statement
            | SyntaxRoleClass::Argument
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
    pub(crate) fn required_exact_child<C: AstKind>(
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

    pub(crate) fn optional_exact_child<C: AstKind>(
        &self,
        role: SyntaxRole,
    ) -> Result<Option<AstNode<C>>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(role)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub(crate) fn exact_children<C: AstKind>(
        &self,
        role: SyntaxRole,
    ) -> Result<Vec<AstNode<C>>, SyntaxAccessError> {
        self.syntax()
            .children_with_role(role)
            .into_iter()
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .collect()
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

    pub(crate) fn ordered_exact_children<C: AstKind>(
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

impl<F: FamilySpec> FamilyNode<F> {
    pub(crate) fn required_exact_child<C: AstKind>(
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

    pub(crate) fn optional_exact_child<C: AstKind>(
        &self,
        role: SyntaxRole,
    ) -> Result<Option<AstNode<C>>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(role)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub(crate) fn required_family_child<C: FamilySpec>(
        &self,
        role: SyntaxRole,
    ) -> Result<FamilyNode<C>, SyntaxAccessError> {
        let syntax = self.syntax().optional_unique_child(role)?.ok_or(
            SyntaxAccessError::MissingFamilyChild {
                parent: self.id(),
                role,
                expected: C::FAMILY,
            },
        )?;
        FamilyNode::new(syntax)
    }

    pub(crate) fn optional_family_child<C: FamilySpec>(
        &self,
        role: SyntaxRole,
    ) -> Result<Option<FamilyNode<C>>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(role)?
            .map(FamilyNode::new)
            .transpose()
    }

    pub(crate) fn family_children<C: FamilySpec>(
        &self,
        role: SyntaxRole,
    ) -> Result<Vec<FamilyNode<C>>, SyntaxAccessError> {
        self.syntax()
            .children_with_role(role)
            .into_iter()
            .map(FamilyNode::new)
            .collect()
    }

    pub(crate) fn ordered_exact_children<C: AstKind>(
        &self,
        role: SyntaxRoleClass,
    ) -> Result<Vec<AstNode<C>>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(role)?
            .into_iter()
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .collect()
    }

    pub(crate) fn ordered_family_children<C: FamilySpec>(
        &self,
        role: SyntaxRoleClass,
    ) -> Result<Vec<FamilyNode<C>>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(role)?
            .into_iter()
            .map(FamilyNode::new)
            .collect()
    }
}

impl AstNode<SourceFileKind> {
    pub(crate) fn items(&self) -> Result<Vec<ItemNode>, SyntaxAccessError> {
        self.ordered_family_children::<ItemFamily>(SyntaxRoleClass::Element)
    }
}

/// Authored declaration body or an exact missing-body recovery node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DeclarationBodyNode {
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

    pub(crate) fn range(&self) -> arcweft_source::SourceRange {
        match self {
            Self::Body(body) => body.range(),
            Self::Missing(body) => body.range(),
        }
    }
}

impl ItemNode {
    fn prefix_owner(&self) -> Result<SyntaxNodeHandle, SyntaxAccessError> {
        let owner = self.syntax();
        Ok(match owner.optional_unique_child(SyntaxRole::Element(0))? {
            Some(header) if header.kind() == SyntaxKind::DeclarationHeader => header,
            _ => owner,
        })
    }

    pub(crate) fn declaration_header(
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

    pub(crate) fn documentation(&self) -> Result<Option<AstNode<DocBlockKind>>, SyntaxAccessError> {
        self.prefix_owner()?
            .optional_unique_child(SyntaxRole::Documentation)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub(crate) fn attributes(&self) -> Result<Vec<AstNode<OuterAttributeKind>>, SyntaxAccessError> {
        self.prefix_owner()?
            .ordered_children(SyntaxRoleClass::Attribute)?
            .into_iter()
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .collect()
    }

    pub(crate) fn visibility(&self) -> Result<Option<AstNode<VisibilityKind>>, SyntaxAccessError> {
        self.prefix_owner()?
            .optional_unique_child(SyntaxRole::Visibility)?
            .map(|syntax| syntax.cast().map_err(SyntaxAccessError::from))
            .transpose()
    }

    pub(crate) fn name(&self) -> Result<Option<NameNode>, SyntaxAccessError> {
        self.prefix_owner()?
            .optional_unique_child(SyntaxRole::Name)?
            .map(FamilyNode::<NameFamily>::new)
            .transpose()
    }

    pub(crate) fn body(&self) -> Result<Option<DeclarationBodyNode>, SyntaxAccessError> {
        self.syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .map(DeclarationBodyNode::new)
            .transpose()
    }

    pub(crate) fn recovery(&self) -> Result<Vec<RecoveryNode>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(SyntaxRoleClass::Recovery)?
            .into_iter()
            .map(FamilyNode::<RecoveryFamily>::new)
            .collect()
    }
}

impl AstNode<DeclarationHeaderKind> {
    pub(crate) fn documentation(&self) -> Result<Option<AstNode<DocBlockKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Documentation)
    }

    pub(crate) fn attributes(&self) -> Result<Vec<AstNode<OuterAttributeKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Attribute)
    }

    pub(crate) fn visibility(&self) -> Result<Option<AstNode<VisibilityKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Visibility)
    }

    pub(crate) fn name(&self) -> Result<Option<NameNode>, SyntaxAccessError> {
        self.optional_family_child::<NameFamily>(SyntaxRole::Name)
    }
}

impl AstNode<PredicateBodyKind> {
    pub(crate) fn content(&self) -> Result<DeclarationBodyNode, SyntaxAccessError> {
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
    pub(crate) fn content(&self) -> Result<DeclarationBodyNode, SyntaxAccessError> {
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
    pub(crate) fn expression(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Body)
    }
}

/// Authored block tail or the exact zero-width omitted-tail marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BlockTailNode {
    Expression(ExprNode),
    Omitted(AstNode<OmittedBlockTailKind>),
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

    pub(crate) fn range(&self) -> arcweft_source::SourceRange {
        match self {
            Self::Expression(expression) => expression.range(),
            Self::Omitted(omitted) => omitted.range(),
        }
    }
}

macro_rules! impl_block_access {
    ($kind:ty) => {
        impl AstNode<$kind> {
            pub(crate) fn open_delimiter(&self) -> Result<DelimiterNode, SyntaxAccessError> {
                self.required_family_child::<DelimiterFamily>(SyntaxRole::OpenDelimiter)
            }

            pub(crate) fn statements(&self) -> Result<Vec<StatementNode>, SyntaxAccessError> {
                self.ordered_family_children::<StatementFamily>(SyntaxRoleClass::Statement)
            }

            pub(crate) fn tail(&self) -> Result<BlockTailNode, SyntaxAccessError> {
                let syntax = self
                    .syntax()
                    .optional_unique_child(SyntaxRole::Tail)?
                    .ok_or(SyntaxAccessError::MissingFamilyChild {
                        parent: self.id(),
                        role: SyntaxRole::Tail,
                        expected: AstNodeFamily::Expression,
                    })?;
                BlockTailNode::new(syntax)
            }

            pub(crate) fn close_delimiter(&self) -> Result<DelimiterNode, SyntaxAccessError> {
                self.required_family_child::<DelimiterFamily>(SyntaxRole::CloseDelimiter)
            }
        }
    };
}

impl_block_access!(PredicateBlockKind);
impl_block_access!(ProofBlockKind);

impl AstNode<AssertionStatementKind> {
    pub(crate) fn conditions(&self) -> Result<Vec<ExprNode>, SyntaxAccessError> {
        self.family_children::<ExpressionFamily>(SyntaxRole::Condition)
    }
}

impl AstNode<LetStatementKind> {
    pub(crate) fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }

    pub(crate) fn annotation(&self) -> Result<Option<TypeNode>, SyntaxAccessError> {
        self.optional_family_child::<TypeFamily>(SyntaxRole::Type)
    }

    pub(crate) fn initializer(&self) -> Result<Option<ExprNode>, SyntaxAccessError> {
        self.optional_family_child::<ExpressionFamily>(SyntaxRole::Initializer)
    }
}

impl AstNode<ProofCallStatementKind> {
    pub(crate) fn callee(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Callee)
    }
}

impl AstNode<CallExpressionKind> {
    pub(crate) fn callee(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Callee)
    }

    pub(crate) fn arguments(&self) -> Result<Vec<AstNode<CallArgumentKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Argument)
    }
}

impl AstNode<CallArgumentKind> {
    pub(crate) fn name(&self) -> Result<Option<AstNode<NameReferenceKind>>, SyntaxAccessError> {
        self.optional_exact_child(SyntaxRole::Name)
    }

    pub(crate) fn operand(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::Operand)
    }
}

impl AstNode<DialogueCallExpressionKind> {
    /// Ordinary call arguments already attached below this dialogue call.
    ///
    /// This remains empty for rich-text payload arguments until that payload
    /// joins the bound grammar in the atomic `ParsedSource` switch. It never
    /// reparses bracket text or manufactures range-backed shadow nodes.
    pub(crate) fn attached_arguments(
        &self,
    ) -> Result<Vec<AstNode<CallArgumentKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Argument)
    }
}

impl AstNode<BinaryExpressionKind> {
    pub(crate) fn left(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::LeftOperand)
    }

    pub(crate) fn right(&self) -> Result<ExprNode, SyntaxAccessError> {
        self.required_family_child::<ExpressionFamily>(SyntaxRole::RightOperand)
    }
}

impl AstNode<ParameterKind> {
    pub(crate) fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::ParameterPattern)
    }

    pub(crate) fn ty(&self) -> Result<TypeNode, SyntaxAccessError> {
        self.required_family_child::<TypeFamily>(SyntaxRole::ParameterType)
    }
}

impl AstNode<FixedParameterGroupKind> {
    pub(crate) fn parameters(&self) -> Result<Vec<AstNode<ParameterKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Parameter)
    }
}

impl AstNode<WholeBindingPatternKind> {
    pub(crate) fn pattern(&self) -> Result<PatternNode, SyntaxAccessError> {
        self.required_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }
}

impl AstNode<RecordPatternKind> {
    pub(crate) fn fields(&self) -> Result<Vec<PatternNode>, SyntaxAccessError> {
        self.ordered_family_children::<PatternFamily>(SyntaxRoleClass::Field)
    }
}

impl AstNode<RecordPatternFieldKind> {
    pub(crate) fn pattern(&self) -> Result<Option<PatternNode>, SyntaxAccessError> {
        self.optional_family_child::<PatternFamily>(SyntaxRole::Pattern)
    }
}

impl AstNode<GenericApplicationTypeKind> {
    pub(crate) fn arguments(&self) -> Result<Vec<AstNode<TypeArgumentKind>>, SyntaxAccessError> {
        self.ordered_exact_children(SyntaxRoleClass::Argument)
    }
}

impl AstNode<TypeArgumentKind> {
    pub(crate) fn ty(&self) -> Result<TypeNode, SyntaxAccessError> {
        self.required_family_child::<TypeFamily>(SyntaxRole::Type)
    }
}

impl AstNode<FunctionTypeKind> {
    pub(crate) fn parameters(&self) -> Result<Vec<TypeNode>, SyntaxAccessError> {
        self.ordered_family_children::<TypeFamily>(SyntaxRoleClass::Element)
    }

    pub(crate) fn result(&self) -> Result<TypeNode, SyntaxAccessError> {
        self.required_family_child::<TypeFamily>(SyntaxRole::RightOperand)
    }
}

#[cfg(test)]
mod tests {
    use super::{DeclarationBodyNode, TypedSyntaxTree};
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
            _tree: Option<TypedSyntaxTree>,
            _body_union: Option<DeclarationBodyNode>,
        ) {
        }
        consume(None, None, None, None, None, None, None);
    }
}
