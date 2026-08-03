//! Typed Signal declaration ownership over the attached grammar tree.

use super::family::RecoveryFamily;
use super::node::{
    AstNode, ColonKind, DeclarationHeaderKind, SignalDeclarationItemKind, SignalObservableTypeKind,
};
use super::nominal::{punctuation, required_type};
use super::{
    AttachedItemPrefix, AttachedRequiredPunctuation, AttachedRetainedHeader, AttachedTypeRefNode,
    RecoveryNode, SyntaxAccessError, TypedItemNode,
};
use crate::grammar::kinds::{SyntaxRole, SyntaxRoleClass};

/// One source-bound Signal declaration with no fabricated initializer payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSignalDeclaration {
    syntax: AstNode<SignalDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    colon: AttachedRequiredPunctuation,
    observable_type: AttachedTypeRefNode,
    trailing_recovery: Option<RecoveryNode>,
}

impl AttachedSignalDeclaration {
    pub const fn syntax(&self) -> &AstNode<SignalDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn observable_type(&self) -> &AttachedTypeRefNode {
        &self.observable_type
    }

    pub const fn trailing_recovery(&self) -> Option<&RecoveryNode> {
        self.trailing_recovery.as_ref()
    }
}

impl AstNode<SignalDeclarationItemKind> {
    /// Binds one retained Signal header and its observable type without a
    /// detached surface reader or source-text rediscovery.
    pub fn semantics(&self) -> Result<AttachedSignalDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Signal(self.clone());
        let header_syntax =
            self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        let observable_type_syntax =
            header_syntax.required_exact_child::<SignalObservableTypeKind>(SyntaxRole::Type)?;
        let recoveries =
            self.ordered_family_children::<RecoveryFamily>(SyntaxRoleClass::Recovery)?;
        let trailing_recovery = match recoveries.as_slice() {
            [] => None,
            [recovery] => Some(recovery.clone()),
            _ => return Err(SyntaxAccessError::InvalidItemProjection { id: self.id() }),
        };

        Ok(AttachedSignalDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header: header_syntax.retained_semantics()?,
            colon: punctuation(
                &header_syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?,
            ),
            observable_type: required_type(&observable_type_syntax.syntax(), SyntaxRole::Type)?,
            trailing_recovery,
        })
    }
}
