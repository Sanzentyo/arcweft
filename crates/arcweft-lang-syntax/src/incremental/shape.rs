//! Deterministic attached-grammar shapes for syntax identity reconciliation.

use std::collections::HashSet;

use crate::attachment::{SyntaxNode, grammar_node_at_path};
use crate::grammar::build::{GrammarBuild, GrammarEventPath};
use crate::grammar::event::ExpectedToken;
use crate::grammar::kinds::{
    SyntaxKind as GrammarKind, SyntaxRole as GrammarRole, SyntaxRoleClass,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum RecoveryClass {
    None,
    Missing(Option<ExpectedToken>),
    Omitted,
    Error,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct GrammarOwnShape {
    kind: GrammarKind,
    role: SyntaxRoleClass,
    own_non_trivia_digest: [u8; 16],
    ordered_child_role_digest: [u8; 16],
    recovery_class: RecoveryClass,
}

#[derive(Clone, Debug)]
pub(super) struct GrammarShapeNode {
    path: GrammarEventPath,
    own: GrammarOwnShape,
    children: Vec<Self>,
    digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GrammarShapeError {
    MissingRoot,
    InvalidEventPath,
    InvalidParentOrder,
    ChildIndexExhausted,
}

impl GrammarShapeNode {
    pub(super) fn from_build(build: &GrammarBuild) -> Result<Self, GrammarShapeError> {
        let entries = build.index().entries();
        if entries
            .first()
            .is_none_or(|entry| entry.kind() != GrammarKind::SourceFile)
        {
            return Err(GrammarShapeError::MissingRoot);
        }
        let root = SyntaxNode::new_root(build.green().clone());
        let identity_paths = entries
            .iter()
            .map(|entry| entry.path().clone())
            .collect::<HashSet<_>>();
        let mut drafts = Vec::<GrammarShapeDraft>::with_capacity(entries.len());
        let mut ancestors = Vec::<usize>::new();

        for entry in entries {
            while ancestors.last().is_some_and(|&candidate| {
                !strict_path_prefix(drafts[candidate].path.elements(), entry.path().elements())
            }) {
                ancestors.pop();
            }
            let parent = ancestors.last().copied();
            if parent.is_none() && !drafts.is_empty() {
                return Err(GrammarShapeError::InvalidParentOrder);
            }
            let syntax = grammar_node_at_path(&root, entry.path())
                .ok_or(GrammarShapeError::InvalidEventPath)?;
            let own_non_trivia_digest = own_token_digest(&syntax, entry.path(), &identity_paths)?;
            let recovery_class = recovery_class(build, entry.kind(), entry.path());
            let index = drafts.len();
            drafts.push(GrammarShapeDraft {
                path: entry.path().clone(),
                kind: entry.kind(),
                role: entry.role(),
                own_non_trivia_digest,
                recovery_class,
                children: Vec::new(),
            });
            if let Some(parent) = parent {
                drafts[parent].children.push(index);
            }
            ancestors.push(index);
        }

        build_grammar_shape(0, &drafts)
    }

    pub(super) const fn path(&self) -> &GrammarEventPath {
        &self.path
    }

    pub(super) const fn role_class(&self) -> SyntaxRoleClass {
        self.own.role
    }

    pub(super) const fn own(&self) -> &GrammarOwnShape {
        &self.own
    }

    pub(super) fn children(&self) -> &[Self] {
        &self.children
    }

    pub(super) const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub(super) fn exactly_equals(&self, other: &Self) -> bool {
        self.own == other.own
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(left, right)| left.exactly_equals(right))
    }
}

#[derive(Clone, Debug)]
struct GrammarShapeDraft {
    path: GrammarEventPath,
    kind: GrammarKind,
    role: GrammarRole,
    own_non_trivia_digest: [u8; 16],
    recovery_class: RecoveryClass,
    children: Vec<usize>,
}

fn build_grammar_shape(
    index: usize,
    drafts: &[GrammarShapeDraft],
) -> Result<GrammarShapeNode, GrammarShapeError> {
    let draft = drafts
        .get(index)
        .ok_or(GrammarShapeError::InvalidParentOrder)?;
    let children = draft
        .children
        .iter()
        .map(|&child| build_grammar_shape(child, drafts))
        .collect::<Result<Vec<_>, _>>()?;

    let mut child_roles = blake3::Hasher::new();
    child_roles.update(b"arcweft-grammar-child-roles-v1\0");
    for child in &children {
        hash_u16(&mut child_roles, child.own.kind as u16);
        hash_u16(&mut child_roles, role_class_tag(child.role_class()));
    }
    let child_role_hash = child_roles.finalize();
    let mut ordered_child_role_digest = [0_u8; 16];
    ordered_child_role_digest.copy_from_slice(&child_role_hash.as_bytes()[..16]);

    let own = GrammarOwnShape {
        kind: draft.kind,
        role: draft.role.class(),
        own_non_trivia_digest: draft.own_non_trivia_digest,
        ordered_child_role_digest,
        recovery_class: draft.recovery_class,
    };
    let mut full = blake3::Hasher::new();
    hash_grammar_own_shape(&mut full, &own);
    for child in &children {
        full.update(&child.digest);
    }
    let digest = *full.finalize().as_bytes();

    Ok(GrammarShapeNode {
        path: draft.path.clone(),
        own,
        children,
        digest,
    })
}

fn own_token_digest(
    syntax: &SyntaxNode,
    path: &GrammarEventPath,
    identity_paths: &HashSet<GrammarEventPath>,
) -> Result<[u8; 16], GrammarShapeError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft-grammar-owned-tokens-v1\0");
    hash_owned_tokens(
        syntax,
        path.elements().to_vec(),
        identity_paths,
        &mut hasher,
    )?;
    let hash = hasher.finalize();
    let mut digest = [0_u8; 16];
    digest.copy_from_slice(&hash.as_bytes()[..16]);
    Ok(digest)
}

fn hash_owned_tokens(
    syntax: &SyntaxNode,
    mut path: Vec<u32>,
    identity_paths: &HashSet<GrammarEventPath>,
    hasher: &mut blake3::Hasher,
) -> Result<(), GrammarShapeError> {
    for (index, element) in syntax.children_with_tokens().enumerate() {
        let index = u32::try_from(index).map_err(|_| GrammarShapeError::ChildIndexExhausted)?;
        path.push(index);
        match element {
            rowan::NodeOrToken::Node(child) => {
                let child_path = GrammarEventPath::from_elements(path.clone().into_boxed_slice());
                if !identity_paths.contains(&child_path) {
                    hash_owned_tokens(&child, path.clone(), identity_paths, hasher)?;
                }
            }
            rowan::NodeOrToken::Token(token) => {
                if is_semantic_grammar_token(token.kind()) {
                    hash_u16(hasher, token.kind().0);
                    hash_field(hasher, token.text().as_bytes());
                }
            }
        }
        path.pop();
    }
    Ok(())
}

fn recovery_class(
    build: &GrammarBuild,
    kind: GrammarKind,
    path: &GrammarEventPath,
) -> RecoveryClass {
    if kind.is_missing_node() {
        let expected = build
            .missing_tokens()
            .iter()
            .find(|site| site.owner_path() == path)
            .map(crate::grammar::build::MissingTokenSite::expected);
        RecoveryClass::Missing(expected)
    } else if kind.is_omitted_node() {
        RecoveryClass::Omitted
    } else if kind.is_error_node() {
        RecoveryClass::Error
    } else {
        RecoveryClass::None
    }
}

fn hash_grammar_own_shape(hasher: &mut blake3::Hasher, shape: &GrammarOwnShape) {
    hasher.update(b"arcweft-grammar-own-shape-v1\0");
    hash_u16(hasher, shape.kind as u16);
    hash_u16(hasher, role_class_tag(shape.role));
    hasher.update(&shape.own_non_trivia_digest);
    hasher.update(&shape.ordered_child_role_digest);
    match shape.recovery_class {
        RecoveryClass::None => {
            hasher.update(&[0]);
        }
        RecoveryClass::Missing(None) => {
            hasher.update(&[1]);
        }
        RecoveryClass::Missing(Some(expected)) => {
            hasher.update(&[2]);
            hash_u16(hasher, expected.kind() as u16);
        }
        RecoveryClass::Omitted => {
            hasher.update(&[3]);
        }
        RecoveryClass::Error => {
            hasher.update(&[4]);
        }
    }
}

fn strict_path_prefix(parent: &[u32], child: &[u32]) -> bool {
    parent.len() < child.len() && child.starts_with(parent)
}

const fn is_semantic_grammar_token(kind: rowan::SyntaxKind) -> bool {
    !matches!(
        kind.0,
        value if value == GrammarKind::WhitespaceToken as u16
            || value == GrammarKind::NewlineToken as u16
            || value == GrammarKind::CommentToken as u16
    )
}

fn hash_u16(hasher: &mut blake3::Hasher, value: u16) {
    hasher.update(&value.to_le_bytes());
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

const fn role_class_tag(role: SyntaxRoleClass) -> u16 {
    role as u16
}
