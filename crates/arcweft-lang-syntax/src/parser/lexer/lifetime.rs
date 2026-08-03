//! Typed projection for one lexer-owned runtime lifetime-registry token.

use crate::expressions::{
    ExpressionComponentRole, PendingExpressionComponent, SyntaxLifetimeRegistryPath,
    SyntaxLifetimeRegistryScope,
};
use crate::grammar::kinds::SyntaxKind;
use crate::name::{SyntaxName, SyntaxNameIssue};

use super::{LexToken, token_local_range};

/// One lifetime-registry projection selected in the lexer transaction.
pub(in crate::parser) struct LifetimeRegistryLexemeProjection {
    syntax: SyntaxLifetimeRegistryPath,
    components: Vec<PendingExpressionComponent>,
}

impl LifetimeRegistryLexemeProjection {
    pub(in crate::parser) fn components(&self) -> &[PendingExpressionComponent] {
        &self.components
    }

    pub(in crate::parser) fn into_syntax(self) -> SyntaxLifetimeRegistryPath {
        self.syntax
    }
}

/// Projects a complete lifetime-registry token without reparsing a substring.
pub(in crate::parser) fn typed_lifetime_registry_path(
    token: LexToken,
    spelling: &str,
) -> LifetimeRegistryLexemeProjection {
    debug_assert_eq!(token.kind(), SyntaxKind::LifetimeToken);
    debug_assert_eq!(
        spelling.len(),
        token.range().end().saturating_sub(token.range().start())
    );
    debug_assert!(spelling.starts_with('\''));

    let optional = spelling.ends_with('?');
    let path_end = spelling.len().saturating_sub(usize::from(optional));
    let body_start = '\''.len_utf8().min(path_end);
    let body = &spelling[body_start..path_end];
    let mut parts = body.split('.');
    let scope_spelling = parts.next().unwrap_or("");
    let scope_end = body_start + scope_spelling.len();
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::LifetimeScope,
        token_local_range(token, 0, scope_end),
    )];
    let scope = SyntaxLifetimeRegistryScope::from_name(SyntaxName::try_new(scope_spelling));

    let mut segments = Vec::new();
    let mut segment_start = scope_end;
    for (ordinal, spelling) in parts.enumerate() {
        segment_start = segment_start
            .checked_add('.'.len_utf8())
            .expect("a token-local segment offset fits the source document");
        let segment_end = segment_start
            .checked_add(spelling.len())
            .expect("a token-local segment end fits the source document");
        let ordinal =
            u32::try_from(ordinal).expect("source document limits bound lifetime key ordinals");
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::LifetimeKeySegment { ordinal },
            token_local_range(token, segment_start, segment_end),
        ));
        segments.push(if spelling.is_empty() {
            Err(SyntaxNameIssue::Missing)
        } else {
            SyntaxName::try_new(spelling)
        });
        segment_start = segment_end;
    }
    if optional {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::LifetimeOptionalMarker,
            token_local_range(token, path_end, spelling.len()),
        ));
    }

    LifetimeRegistryLexemeProjection {
        syntax: SyntaxLifetimeRegistryPath::new(scope, segments, optional),
        components,
    }
}
