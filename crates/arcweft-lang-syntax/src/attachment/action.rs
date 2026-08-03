//! Typed Action declaration ownership over the attached grammar tree.

use super::family::{ExpressionFamily, PatternFamily, TypeFamily};
use super::node::{
    ActionDeclarationItemKind, ActionSignatureKind, AstNode, CloseParenKind, ColonKind,
    DeclarationHeaderKind, EqualsKind, ErrorNodeKind, FixedParameterGroupKind, OpenParenKind,
    ParameterKind,
};
use super::nominal::punctuation;
use super::{
    AttachedExpressionNode, AttachedItemPrefix, AttachedPatternNode, AttachedRequiredPunctuation,
    AttachedRetainedHeader, AttachedTypeFamily, AttachedTypeRefNode, SyntaxAccessError,
    TypedItemNode,
};
use crate::grammar::kinds::{SyntaxRole, SyntaxRoleClass};
use crate::patterns::PatternSyntaxFamily;

/// One forbidden Action parameter default retained as typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActionForbiddenDefault {
    syntax: AstNode<ErrorNodeKind>,
    equals: AstNode<EqualsKind>,
    value: AttachedExpressionNode,
}

impl AttachedActionForbiddenDefault {
    pub const fn syntax(&self) -> &AstNode<ErrorNodeKind> {
        &self.syntax
    }

    pub const fn equals(&self) -> &AstNode<EqualsKind> {
        &self.equals
    }

    pub const fn value(&self) -> &AttachedExpressionNode {
        &self.value
    }
}

/// One source-ordered Action channel parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActionParameter {
    syntax: AstNode<ParameterKind>,
    source_ordinal: u16,
    pattern: AttachedPatternNode,
    colon: AttachedRequiredPunctuation,
    ty: AttachedTypeRefNode,
    forbidden_default: Option<AttachedActionForbiddenDefault>,
}

impl AttachedActionParameter {
    pub const fn syntax(&self) -> &AstNode<ParameterKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn ty(&self) -> &AttachedTypeRefNode {
        &self.ty
    }

    pub const fn forbidden_default(&self) -> Option<&AttachedActionForbiddenDefault> {
        self.forbidden_default.as_ref()
    }

    /// Whether the retained Pattern is not one clean ordinary binding name.
    pub fn has_invalid_binding(&self) -> bool {
        self.pattern.family() != PatternSyntaxFamily::Binding || !self.pattern.state().is_valid()
    }

    pub fn has_recovery(&self) -> bool {
        self.has_invalid_binding()
            || self.colon.is_missing()
            || self.ty.family() == AttachedTypeFamily::Recovery
            || self.forbidden_default.is_some()
    }
}

/// The sole fixed, bodyless signature owned by one Action declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActionSignature {
    syntax: AstNode<ActionSignatureKind>,
    parameter_group: AstNode<FixedParameterGroupKind>,
    open: AstNode<OpenParenKind>,
    open_state: AttachedRequiredPunctuation,
    close: AstNode<CloseParenKind>,
    close_state: AttachedRequiredPunctuation,
    parameters: Box<[AttachedActionParameter]>,
}

impl AttachedActionSignature {
    pub const fn syntax(&self) -> &AstNode<ActionSignatureKind> {
        &self.syntax
    }

    pub const fn parameter_group(&self) -> &AstNode<FixedParameterGroupKind> {
        &self.parameter_group
    }

    pub const fn open(&self) -> &AstNode<OpenParenKind> {
        &self.open
    }

    pub const fn open_state(&self) -> &AttachedRequiredPunctuation {
        &self.open_state
    }

    pub const fn close(&self) -> &AstNode<CloseParenKind> {
        &self.close
    }

    pub const fn close_state(&self) -> &AttachedRequiredPunctuation {
        &self.close_state
    }

    pub const fn parameters(&self) -> &[AttachedActionParameter] {
        &self.parameters
    }

    pub fn has_recovery(&self) -> bool {
        self.open_state.is_missing()
            || self.close_state.is_missing()
            || self
                .parameters
                .iter()
                .any(AttachedActionParameter::has_recovery)
    }
}

/// Forbidden return, body, or other tail owned by the declaration's recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActionTrailingRecovery {
    syntax: AstNode<ErrorNodeKind>,
}

impl AttachedActionTrailingRecovery {
    pub const fn syntax(&self) -> &AstNode<ErrorNodeKind> {
        &self.syntax
    }
}

/// One source-bound bodyless Action channel declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActionDeclaration {
    syntax: AstNode<ActionDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    signature: AttachedActionSignature,
    trailing_recovery: Option<AttachedActionTrailingRecovery>,
}

impl AttachedActionDeclaration {
    pub const fn syntax(&self) -> &AstNode<ActionDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn signature(&self) -> &AttachedActionSignature {
        &self.signature
    }

    pub const fn trailing_recovery(&self) -> Option<&AttachedActionTrailingRecovery> {
        self.trailing_recovery.as_ref()
    }
}

impl AstNode<ActionDeclarationItemKind> {
    /// Binds one retained Action header and its sole typed signature without a
    /// detached declaration reader or source-text rediscovery.
    pub fn semantics(&self) -> Result<AttachedActionDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Action(self.clone());
        let header_syntax =
            self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        let signature = header_syntax
            .required_exact_child::<ActionSignatureKind>(SyntaxRole::ParameterGroup)?
            .semantics()?;
        let recoveries = self.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
        let trailing_recovery = match recoveries.as_slice() {
            [] => None,
            [syntax] => Some(AttachedActionTrailingRecovery {
                syntax: syntax.clone(),
            }),
            _ => return Err(SyntaxAccessError::InvalidItemProjection { id: self.id() }),
        };

        Ok(AttachedActionDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header: header_syntax.retained_semantics()?,
            signature,
            trailing_recovery,
        })
    }
}

impl AstNode<ActionSignatureKind> {
    /// Binds the exact fixed parameter group selected by the Action grammar.
    pub fn semantics(&self) -> Result<AttachedActionSignature, SyntaxAccessError> {
        let parameter_group =
            self.required_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)?;
        let open =
            parameter_group.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?;
        let close =
            parameter_group.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?;
        let parameters = parameter_group
            .parameters()?
            .into_iter()
            .enumerate()
            .map(|(ordinal, syntax)| {
                attach_parameter(
                    syntax,
                    u16::try_from(ordinal)
                        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: self.id() })?,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        Ok(AttachedActionSignature {
            syntax: self.clone(),
            parameter_group,
            open_state: punctuation(&open),
            open,
            close_state: punctuation(&close),
            close,
            parameters,
        })
    }
}

fn attach_parameter(
    syntax: AstNode<ParameterKind>,
    source_ordinal: u16,
) -> Result<AttachedActionParameter, SyntaxAccessError> {
    if syntax.role() != SyntaxRole::Parameter(source_ordinal) {
        return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
    }
    let pattern = syntax
        .required_family_child::<PatternFamily>(SyntaxRole::ParameterPattern)?
        .semantic()?;
    let colon = punctuation(&syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?);
    let ty = syntax
        .required_family_child::<TypeFamily>(SyntaxRole::ParameterType)?
        .semantic()?;
    let recoveries = syntax.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
    let forbidden_default = match recoveries.as_slice() {
        [] => None,
        [recovery] => Some(AttachedActionForbiddenDefault {
            syntax: recovery.clone(),
            equals: recovery.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
            value: recovery
                .required_family_child::<ExpressionFamily>(SyntaxRole::Initializer)?
                .semantic()?,
        }),
        _ => return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() }),
    };

    Ok(AttachedActionParameter {
        syntax,
        source_ordinal,
        pattern,
        colon,
        ty,
        forbidden_default,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{ActionDeclarationItemKind, AstNode, AttachedRequiredPunctuation};
    use crate::attachment::{
        GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
        SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::grammar::kinds::SyntaxKind;
    use crate::parser::{ParseOptions, parse_shadow_document};
    use crate::patterns::PatternSyntaxFamily;

    fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/action-attachment-test").unwrap(),
                SourceName::path("action-attachment-test.arcw"),
                text,
            )
            .unwrap(),
        );
        let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(131).unwrap());
        let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
        let snapshot = SyntaxSnapshotId::new(
            lineage,
            SourceSnapshotId::initial(document.display_name().clone()),
        );
        let identities = build
            .index()
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    entry.path().clone(),
                    SyntaxNodeId::new(
                        lineage,
                        NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        attach_typed_tree(
            &build,
            &GrammarIdentityMap::new(identities),
            snapshot,
            document,
        )
        .unwrap()
    }

    fn actions(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<ActionDeclarationItemKind>> {
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::ActionDeclarationItem)
            .map(|node| node.cast().unwrap())
            .collect()
    }

    #[test]
    fn action_attachment_owns_prefix_header_and_ordered_typed_signature() {
        let snapshot = attach(concat!(
            "/// Sends feedback\n",
            "#[test.fixture]\n",
            "pub action @action.feedback_submit feedback_submit(value: Feedback, count: Count);\n",
        ));
        let declaration = actions(&snapshot)[0].semantics().unwrap();

        assert_eq!(
            declaration.prefix().documentation().unwrap().markdown(),
            "Sends feedback"
        );
        assert_eq!(declaration.prefix().attributes().len(), 1);
        assert!(declaration.prefix().visibility().is_some());
        assert!(declaration.trailing_recovery().is_none());
        let signature = declaration.signature();
        assert!(!signature.open_state().is_missing());
        assert!(!signature.close_state().is_missing());
        assert_eq!(signature.parameters().len(), 2);
        for (ordinal, parameter) in signature.parameters().iter().enumerate() {
            assert_eq!(usize::from(parameter.source_ordinal()), ordinal);
            assert_eq!(parameter.pattern().family(), PatternSyntaxFamily::Binding);
            assert!(!parameter.has_invalid_binding());
            assert!(!parameter.colon().is_missing());
            assert_eq!(parameter.ty().family(), super::AttachedTypeFamily::Path);
            assert!(parameter.forbidden_default().is_none());
        }
    }

    #[test]
    fn action_attachment_retains_missing_invalid_and_forbidden_shapes() {
        let snapshot = attach(concat!(
            "action Missing\n",
            "action Invalid((left, right): Pair)\n",
            "action Untyped(value)\n",
            "action Defaulted(value: String = make())\n",
            "action Query() -> String\n",
            "action Run() { return }\n",
            "action Tail() effects { ui.write }\n",
        ));
        let declarations = actions(&snapshot)
            .iter()
            .map(AstNode::<ActionDeclarationItemKind>::semantics)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(declarations.len(), 7);

        let missing = declarations[0].signature();
        assert!(missing.open_state().is_missing());
        assert!(missing.close_state().is_missing());
        assert!(missing.parameters().is_empty());

        assert!(declarations[1].signature().parameters()[0].has_invalid_binding());
        let untyped = &declarations[2].signature().parameters()[0];
        assert!(matches!(
            untyped.colon(),
            AttachedRequiredPunctuation::Missing(_)
        ));
        assert_eq!(untyped.ty().family(), super::AttachedTypeFamily::Recovery);

        let default = declarations[3].signature().parameters()[0]
            .forbidden_default()
            .unwrap();
        assert_eq!(default.equals().source_text(), "=");
        assert_eq!(default.value().syntax().source_text(), "make()");

        for declaration in &declarations[4..] {
            assert!(declaration.trailing_recovery().is_some());
        }
        assert_eq!(
            declarations[4]
                .trailing_recovery()
                .unwrap()
                .syntax()
                .source_text(),
            "-> String"
        );
        assert_eq!(
            declarations[5]
                .trailing_recovery()
                .unwrap()
                .syntax()
                .source_text(),
            "{ return }"
        );
        assert_eq!(
            declarations[6]
                .trailing_recovery()
                .unwrap()
                .syntax()
                .source_text(),
            "effects { ui.write }"
        );
    }
}
