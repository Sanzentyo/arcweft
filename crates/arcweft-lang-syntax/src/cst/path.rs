//! Typed path-root projection from the lossless CST.

use super::{RowanTextRange, SyntaxKind, SyntaxNode};

/// Source spelling of a rooted Arcweft path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CstPathRootKind {
    /// `crate::`.
    Crate,
    /// `self::`.
    SelfModule,
    /// Canonical `super::`.
    Super,
    /// Authoring alias `parent::`, canonicalized to `super::` by tooling.
    ParentAlias,
}

/// One path root identified from lossless CST tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CstPathRoot {
    kind: CstPathRootKind,
    name_range: RowanTextRange,
}

impl CstPathRoot {
    /// Source spelling of this root.
    pub const fn kind(self) -> CstPathRootKind {
        self.kind
    }

    /// Exact UTF-8 byte range of the root name, excluding `::`.
    pub const fn name_range(self) -> RowanTextRange {
        self.name_range
    }
}

/// Projects rooted-path tokens from a lossless syntax tree.
///
/// String and comment contents are single non-identifier CST tokens and cannot
/// become path roots. The returned ranges remain document-relative byte
/// offsets suitable for source edits.
#[must_use]
pub fn cst_path_roots(root: &SyntaxNode) -> Vec<CstPathRoot> {
    let tokens = root
        .descendants_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect::<Vec<_>>();

    tokens
        .windows(3)
        .filter_map(|window| {
            let [name, first_colon, second_colon] = window else {
                return None;
            };
            if name.kind() != SyntaxKind::Ident
                || first_colon.kind() != SyntaxKind::Punctuation
                || second_colon.kind() != SyntaxKind::Punctuation
                || first_colon.text() != ":"
                || second_colon.text() != ":"
                || name.text_range().end() != first_colon.text_range().start()
                || first_colon.text_range().end() != second_colon.text_range().start()
            {
                return None;
            }
            let kind = match name.text() {
                "crate" => CstPathRootKind::Crate,
                "self" => CstPathRootKind::SelfModule,
                "super" => CstPathRootKind::Super,
                "parent" => CstPathRootKind::ParentAlias,
                _ => return None,
            };
            Some(CstPathRoot {
                kind,
                name_range: name.text_range(),
            })
        })
        .collect()
}
