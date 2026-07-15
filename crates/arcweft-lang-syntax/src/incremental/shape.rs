//! Semantic CST shapes used only for deterministic identity reconciliation.
//!
//! The current lossless CST projects physical lines directly under the root.
//! Reconciliation derives brace and indentation parents over those same nodes
//! so a move across lexical owners cannot look like a root-level reorder. This
//! hierarchy never changes the public CST or creates another syntax identity.

use crate::cst::{CstPunctuationScan, SyntaxKind, SyntaxNode};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct OwnShape {
    kind: SyntaxKind,
    child_roles: Box<[SyntaxKind]>,
    tokens: Box<[TokenShape]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TokenShape {
    kind: SyntaxKind,
    text: String,
}

#[derive(Clone, Debug)]
pub(super) struct ShapeNode {
    syntax: SyntaxNode,
    own: OwnShape,
    children: Vec<ShapeNode>,
    digest: [u8; 32],
}

impl ShapeNode {
    pub(super) fn from_syntax(syntax: SyntaxNode) -> Self {
        let mut root = Self::from_raw_syntax(syntax);
        if root.syntax.kind() == SyntaxKind::Root
            && root
                .children
                .iter()
                .all(|child| child.syntax.kind() == SyntaxKind::Line)
        {
            root.children = nest_flat_lines(core::mem::take(&mut root.children));
            root.refresh_semantic_shape();
        }
        root
    }

    fn from_raw_syntax(syntax: SyntaxNode) -> Self {
        let children = syntax
            .children()
            .map(Self::from_raw_syntax)
            .collect::<Vec<_>>();
        let own = OwnShape {
            kind: syntax.kind(),
            child_roles: children
                .iter()
                .map(|child| child.syntax.kind())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            tokens: syntax
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
                .filter(|token| is_semantic_token(token.kind()))
                .map(|token| TokenShape {
                    kind: token.kind(),
                    text: token.text().to_owned(),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };

        let mut hasher = blake3::Hasher::new();
        hash_own_shape(&mut hasher, &own);
        for child in &children {
            hasher.update(&child.digest);
        }
        let digest = *hasher.finalize().as_bytes();

        Self {
            syntax,
            own,
            children,
            digest,
        }
    }

    fn refresh_semantic_shape(&mut self) {
        for child in &mut self.children {
            child.refresh_semantic_shape();
        }
        // Synthetic block nesting constrains which siblings may reconcile, but
        // it is not another CST child role. Keeping the raw own shape lets an
        // unchanged block owner survive insertion or removal inside its body.
        let mut hasher = blake3::Hasher::new();
        hash_own_shape(&mut hasher, &self.own);
        for child in &self.children {
            hasher.update(&child.digest);
        }
        self.digest = *hasher.finalize().as_bytes();
    }

    pub(super) const fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    pub(super) const fn own(&self) -> &OwnShape {
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
                .zip(other.children.iter())
                .all(|(left, right)| left.exactly_equals(right))
    }
}

fn nest_flat_lines(lines: Vec<ShapeNode>) -> Vec<ShapeNode> {
    let mut roots = Vec::new();
    let mut parents = Vec::<ReconciliationParent>::new();
    let mut previous_line = None::<PreviousSemanticLine>;

    for line in lines {
        let line_text = line.syntax.text().to_string();
        let scan = CstPunctuationScan::new(&line_text);
        let leading_closes = scan.leading_brace_closes();
        let brace_delta = scan.deltas().brace;
        for _ in 0..leading_closes {
            let Some(brace_parent) = parents
                .iter()
                .rposition(|parent| matches!(parent.boundary, ParentBoundary::Brace))
            else {
                break;
            };
            parents.truncate(brace_parent);
        }

        let indentation = line_text
            .bytes()
            .take_while(|byte| matches!(*byte, b' ' | b'\t'))
            .count();
        let trivia = line.own.tokens.is_empty();
        if !trivia {
            while matches!(
                parents.last(),
                Some(ReconciliationParent {
                    boundary: ParentBoundary::Indent { child_indent },
                    ..
                }) if indentation < *child_indent
            ) {
                parents.pop();
            }
            if let Some(previous) = previous_line.as_ref()
                && indentation > previous.indentation
                && !previous.opened_brace
                && previous.opens_indented_block
            {
                parents.push(ReconciliationParent {
                    path: previous.path.clone(),
                    boundary: ParentBoundary::Indent {
                        child_indent: indentation,
                    },
                });
            }
        }

        let parent_path = parents
            .last()
            .map_or(&[][..], |parent| parent.path.as_slice());
        let siblings = children_at_path_mut(&mut roots, parent_path);
        let child_index = siblings.len();
        siblings.push(line);

        let mut inserted_path = parent_path.to_vec();
        inserted_path.push(child_index);
        let block_depth_delta = brace_delta
            .saturating_add(i32::try_from(leading_closes).unwrap_or(i32::MAX))
            .max(0);
        let opens_after_leading_closes = usize::try_from(block_depth_delta).unwrap_or(usize::MAX);
        for _ in 0..opens_after_leading_closes {
            parents.push(ReconciliationParent {
                path: inserted_path.clone(),
                boundary: ParentBoundary::Brace,
            });
        }
        if !trivia {
            previous_line = Some(PreviousSemanticLine {
                path: inserted_path,
                indentation,
                opened_brace: opens_after_leading_closes > 0,
                opens_indented_block: opens_indented_block(&line_text, &scan),
            });
        }
    }

    roots
}

#[derive(Clone, Debug)]
struct ReconciliationParent {
    path: Vec<usize>,
    boundary: ParentBoundary,
}

#[derive(Clone, Copy, Debug)]
enum ParentBoundary {
    Brace,
    Indent { child_indent: usize },
}

#[derive(Clone, Debug)]
struct PreviousSemanticLine {
    path: Vec<usize>,
    indentation: usize,
    opened_brace: bool,
    opens_indented_block: bool,
}

fn opens_indented_block(source: &str, scan: &CstPunctuationScan<'_>) -> bool {
    let trimmed = source.trim();
    trimmed == "defer"
        || scan
            .find_top_level_punctuation(':')
            .is_some_and(|colon| colon + ':'.len_utf8() == source.trim_end().len())
}

fn children_at_path_mut<'a>(
    nodes: &'a mut Vec<ShapeNode>,
    path: &[usize],
) -> &'a mut Vec<ShapeNode> {
    let Some((&index, remaining)) = path.split_first() else {
        return nodes;
    };
    children_at_path_mut(&mut nodes[index].children, remaining)
}

fn hash_own_shape(hasher: &mut blake3::Hasher, shape: &OwnShape) {
    hash_field(hasher, shape.kind.cache_fact_tag().as_bytes());
    for role in &shape.child_roles {
        hash_field(hasher, role.cache_fact_tag().as_bytes());
    }
    for token in &shape.tokens {
        hash_field(hasher, token.kind.cache_fact_tag().as_bytes());
        hash_field(hasher, token.text.as_bytes());
    }
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

const fn is_semantic_token(kind: SyntaxKind) -> bool {
    !matches!(
        kind,
        SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::Comment
    )
}

#[cfg(test)]
mod tests {
    use super::ShapeNode;
    use crate::cst::parse_cst;

    #[test]
    fn ordinary_trivia_is_not_part_of_semantic_shape() {
        let plain = ShapeNode::from_syntax(parse_cst("flow story {}\n"));
        let trivia = ShapeNode::from_syntax(parse_cst("flow  story {} // comment\n"));
        assert!(plain.exactly_equals(&trivia));
        assert_eq!(plain.digest(), trivia.digest());
    }

    #[test]
    fn doc_comments_remain_semantic_shape_tokens() {
        let plain = ShapeNode::from_syntax(parse_cst("flow story {}\n"));
        let documented = ShapeNode::from_syntax(parse_cst("/// story\nflow story {}\n"));
        assert_ne!(plain.digest(), documented.digest());
    }

    #[test]
    fn only_current_grammar_block_heads_own_indented_lines() {
        let continuation = ShapeNode::from_syntax(parse_cst("value\n    .map(f)\n"));
        assert_eq!(continuation.children().len(), 2);

        let recovered = ShapeNode::from_syntax(parse_cst("unknown\n    also_unknown\n"));
        assert_eq!(recovered.children().len(), 2);

        let scope = ShapeNode::from_syntax(parse_cst("scope local:\n    value\n"));
        assert_eq!(scope.children().len(), 1);
        assert_eq!(scope.children()[0].children().len(), 1);
    }
}
