//! Exact-kind family handles over one immutable attached syntax snapshot.
//!
//! A family handle is a navigation convenience, not a cast authority. Every
//! family below owns an explicit concrete-kind predicate; [`AstTag`] alone is
//! never sufficient to construct one.

#![allow(
    dead_code,
    reason = "the private family inventory is consumed only after the atomic ParsedSource syntax switch"
)]

use core::marker::PhantomData;

use arcweft_source::SourceRange;

use super::{
    AstNode, ExactAstKind, SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

/// Coarse family named in structured child-access diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AstNodeFamily {
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

pub(crate) trait FamilySpec: Copy + 'static {
    const FAMILY: AstNodeFamily;

    fn accepts(kind: SyntaxKind) -> bool;
}

/// Snapshot-bound node whose concrete kind belongs to an explicit family.
pub(crate) struct FamilyNode<F: FamilySpec> {
    syntax: SyntaxNodeHandle,
    marker: PhantomData<fn() -> F>,
}

impl<F: FamilySpec> Clone for FamilyNode<F> {
    fn clone(&self) -> Self {
        Self {
            syntax: self.syntax.clone(),
            marker: PhantomData,
        }
    }
}

impl<F: FamilySpec> core::fmt::Debug for FamilyNode<F> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FamilyNode")
            .field("family", &F::FAMILY)
            .field("kind", &self.kind())
            .field("id", &self.id())
            .field("snapshot", self.snapshot_id())
            .finish()
    }
}

impl<F: FamilySpec> PartialEq for FamilyNode<F> {
    fn eq(&self, other: &Self) -> bool {
        self.syntax == other.syntax
    }
}

impl<F: FamilySpec> Eq for FamilyNode<F> {}

impl<F: FamilySpec> FamilyNode<F> {
    pub(super) fn new(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        if !F::accepts(syntax.kind()) {
            return Err(SyntaxAccessError::FamilyMismatch {
                id: syntax.id(),
                expected: F::FAMILY,
                actual_kind: syntax.kind(),
                actual_tag: syntax.tag(),
            });
        }
        Ok(Self {
            syntax,
            marker: PhantomData,
        })
    }

    pub(crate) fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub(crate) fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub(crate) fn kind(&self) -> SyntaxKind {
        self.syntax.kind()
    }

    pub(crate) fn role(&self) -> SyntaxRole {
        self.syntax.role()
    }

    pub(crate) fn range(&self) -> SourceRange {
        self.syntax.range()
    }

    pub(crate) fn cast<K: ExactAstKind>(&self) -> Result<AstNode<K>, super::SyntaxLookupError> {
        self.syntax.cast()
    }
}

macro_rules! define_family {
    ($family:ident, $node:ident, $name:ident, $accepts:expr) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) struct $family;

        impl FamilySpec for $family {
            const FAMILY: AstNodeFamily = AstNodeFamily::$name;

            fn accepts(kind: SyntaxKind) -> bool {
                ($accepts)(kind)
            }
        }

        pub(crate) type $node = FamilyNode<$family>;
    };
}

define_family!(ItemFamily, ItemNode, Item, SyntaxKind::is_item);
define_family!(
    StatementFamily,
    StatementNode,
    Statement,
    SyntaxKind::is_statement
);
define_family!(
    ExpressionFamily,
    ExprNode,
    Expression,
    SyntaxKind::is_expression
);
define_family!(
    PatternFamily,
    PatternNode,
    Pattern,
    SyntaxKind::is_pattern_node
);
define_family!(TypeFamily, TypeNode, Type, SyntaxKind::is_type_node);
define_family!(AttributeFamily, AttributeNode, Attribute, |kind| matches!(
    kind,
    SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute | SyntaxKind::DocBlock
));
define_family!(NameFamily, NameNode, Name, |kind| matches!(
    kind,
    SyntaxKind::NameDefinition | SyntaxKind::NameReference | SyntaxKind::MissingName
));
define_family!(PathFamily, PathNode, Path, |kind| matches!(
    kind,
    SyntaxKind::Path
));
define_family!(
    DeclarationPartFamily,
    DeclarationPartNode,
    DeclarationPart,
    |kind| matches!(
        kind,
        SyntaxKind::Visibility
            | SyntaxKind::DeclarationHeader
            | SyntaxKind::DeclarationPublicId
            | SyntaxKind::SurfaceAlias
            | SyntaxKind::GenericParameterGroup
            | SyntaxKind::GenericParameter
            | SyntaxKind::LifetimeParameter
            | SyntaxKind::TypeParameter
            | SyntaxKind::FixedParameterGroup
            | SyntaxKind::Parameter
            | SyntaxKind::WhereClause
            | SyntaxKind::WherePredicate
            | SyntaxKind::ReturnType
            | SyntaxKind::RequiresClause
            | SyntaxKind::EnsuresClause
            | SyntaxKind::ResourceFieldInitializer
            | SyntaxKind::CharacterDisplayNameMember
            | SyntaxKind::ViewExportBlock
            | SyntaxKind::ViewExportDeclaration
            | SyntaxKind::ViewFragment
            | SyntaxKind::ActionSignature
            | SyntaxKind::ActivityModeMember
            | SyntaxKind::ActivityLifecycleMember
            | SyntaxKind::ActivityInputBlock
            | SyntaxKind::ActivityOutputBlock
            | SyntaxKind::ActivityPort
            | SyntaxKind::ActivityContractBlock
            | SyntaxKind::SignalObservableType
            | SyntaxKind::MetricKind
            | SyntaxKind::MetricUnitMember
            | SyntaxKind::MetricLabelsBlock
            | SyntaxKind::MetricLabel
            | SyntaxKind::MetricBucketsMember
            | SyntaxKind::LayerKindNode
            | SyntaxKind::LayerMember
            | SyntaxKind::LayerPolicyValue
            | SyntaxKind::RetainedReference
            | SyntaxKind::StyleTokenDeclaration
            | SyntaxKind::StyleRule
            | SyntaxKind::StyleSelector
            | SyntaxKind::StyleSelectorSequence
            | SyntaxKind::StylePropertyDeclaration
            | SyntaxKind::StyleEnvironmentBlock
            | SyntaxKind::StyleEnvironmentCondition
            | SyntaxKind::StyleEnvironmentClause
            | SyntaxKind::EntryRoleBinding
            | SyntaxKind::EntryGoto
            | SyntaxKind::EntryRoute
            | SyntaxKind::EntryRouteBinding
            | SyntaxKind::EntryOption
    )
);
define_family!(BodyFamily, BodyNode, Body, |kind| matches!(
    kind,
    SyntaxKind::ExpressionBody
        | SyntaxKind::PredicateBody
        | SyntaxKind::ProofBody
        | SyntaxKind::FunctionBody
        | SyntaxKind::FlowBody
        | SyntaxKind::ResourceBody
        | SyntaxKind::CharacterBody
        | SyntaxKind::ViewDeclarationBody
        | SyntaxKind::ActivityBody
        | SyntaxKind::MetricBody
        | SyntaxKind::LayerBody
        | SyntaxKind::StyleBody
        | SyntaxKind::EntryBody
        | SyntaxKind::Block
        | SyntaxKind::PredicateBlock
        | SyntaxKind::ProofBlock
));
define_family!(DelimiterFamily, DelimiterNode, Delimiter, |kind| matches!(
    kind,
    SyntaxKind::OpenBraceNode
        | SyntaxKind::CloseBraceNode
        | SyntaxKind::OpenParenNode
        | SyntaxKind::CloseParenNode
        | SyntaxKind::OpenBracketNode
        | SyntaxKind::CloseBracketNode
        | SyntaxKind::OpenAngleNode
        | SyntaxKind::CloseAngleNode
));
define_family!(RichTextFamily, RichTextNode, RichText, |kind| matches!(
    kind,
    SyntaxKind::RichTextTag
        | SyntaxKind::RichTextEndTag
        | SyntaxKind::RichTextTagName
        | SyntaxKind::RichTextArgumentPayload
        | SyntaxKind::RichTextFxCallPayload
        | SyntaxKind::RichTextDialogueCallPayload
        | SyntaxKind::RichTextConditionPayload
        | SyntaxKind::RichTextPositionalArgument
        | SyntaxKind::RichTextNamedArgument
        | SyntaxKind::RichTextInvalidArgument
        | SyntaxKind::RichTextArgumentKey
        | SyntaxKind::RichTextArgumentEquals
        | SyntaxKind::RichTextArgumentValue
        | SyntaxKind::RichTextArgumentToken
        | SyntaxKind::RichTextArgumentContent
        | SyntaxKind::RichTextArgumentQuote
        | SyntaxKind::RichTextMissingArgumentValue
        | SyntaxKind::RichTextInvalidArgumentIssue
));
define_family!(
    RecoveryFamily,
    RecoveryNode,
    Recovery,
    |kind: SyntaxKind| {
        kind.is_missing_node()
            || kind.is_error_node()
            || matches!(
                kind,
                SyntaxKind::WrongFamilyReference
                    | SyntaxKind::MissingDeclarationId
                    | SyntaxKind::MissingMemberValue
            )
    }
);

#[cfg(test)]
mod tests {
    use super::{
        AstNodeFamily, AttributeFamily, BodyFamily, DeclarationPartFamily, DelimiterFamily,
        ExpressionFamily, FamilySpec, ItemFamily, NameFamily, PathFamily, PatternFamily,
        RecoveryFamily, RichTextFamily, StatementFamily, TypeFamily,
    };
    use crate::grammar::kinds::{AstTag, IdentityClass, SyntaxKind};

    #[test]
    fn concrete_family_predicates_accept_only_explicit_identity_kinds() {
        for &kind in SyntaxKind::ALL {
            let accepted = [
                (AstNodeFamily::Item, ItemFamily::accepts(kind)),
                (AstNodeFamily::Statement, StatementFamily::accepts(kind)),
                (AstNodeFamily::Expression, ExpressionFamily::accepts(kind)),
                (AstNodeFamily::Pattern, PatternFamily::accepts(kind)),
                (AstNodeFamily::Type, TypeFamily::accepts(kind)),
                (AstNodeFamily::Attribute, AttributeFamily::accepts(kind)),
                (AstNodeFamily::Name, NameFamily::accepts(kind)),
                (AstNodeFamily::Path, PathFamily::accepts(kind)),
                (
                    AstNodeFamily::DeclarationPart,
                    DeclarationPartFamily::accepts(kind),
                ),
                (AstNodeFamily::Body, BodyFamily::accepts(kind)),
                (AstNodeFamily::Delimiter, DelimiterFamily::accepts(kind)),
                (AstNodeFamily::RichText, RichTextFamily::accepts(kind)),
                (AstNodeFamily::Recovery, RecoveryFamily::accepts(kind)),
            ]
            .iter()
            .any(|(_, accepted)| *accepted);
            if accepted {
                assert_eq!(
                    kind.identity_class(),
                    IdentityClass::IdentityBearing,
                    "family accepted structural/token {kind:?}"
                );
            }
        }

        assert_eq!(SyntaxKind::CallArgument.ast_tag(), Some(AstTag::Expression));
        assert!(
            !ExpressionFamily::accepts(SyntaxKind::CallArgument),
            "a coarse expression tag must not cast a call-argument helper to ExprNode"
        );
        assert!(RecoveryFamily::accepts(SyntaxKind::MissingExpression));
        assert!(ExpressionFamily::accepts(SyntaxKind::MissingExpression));
    }
}
