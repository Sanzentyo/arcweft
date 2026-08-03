//! Exact candidate-local authority for `Path` expression interpretation.

use super::{
    AttachedCandidateNode, AttachedCandidatePathProjection, AttachedCandidateTypeProjection,
};
use crate::expressions::ExpressionProjection;
use crate::grammar::{SyntaxKind, SyntaxRole};

/// The one parser-selected interpretation of a candidate `Path` expression.
#[derive(Clone, Copy)]
pub enum AttachedCandidatePathExpression<'a> {
    /// A runtime value path with parser-owned root and segment semantics.
    Value(AttachedCandidatePathProjection<'a>),
    /// A nominal type root retained for dot-associated call resolution.
    NominalType(AttachedCandidateNominalTypeRoot<'a>),
}

/// One exact candidate-local nominal type root.
#[derive(Clone, Copy)]
pub struct AttachedCandidateNominalTypeRoot<'a> {
    node: AttachedCandidateNode<'a>,
    projection: AttachedCandidateTypeProjection<'a>,
}

impl<'a> AttachedCandidateNominalTypeRoot<'a> {
    /// Candidate-local type node selected by the `Path` grammar transaction.
    pub const fn node(self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Typed value and structural path retained by the selected root.
    pub const fn projection(self) -> AttachedCandidateTypeProjection<'a> {
        self.projection
    }
}

impl<'a> AttachedCandidateNode<'a> {
    /// Selects the exact value-path or nominal-type interpretation of this
    /// candidate `Path` expression without consulting source text.
    ///
    /// Returns `None` for every other expression family and for any candidate
    /// graph that does not retain exactly one valid interpretation.
    pub fn path_expression_view(self) -> Option<AttachedCandidatePathExpression<'a>> {
        if self.expression_projection() != Some(&ExpressionProjection::Path) {
            return None;
        }

        let paths = self
            .children()
            .filter(|child| child.path_projection().is_some())
            .collect::<Vec<_>>();
        let types = self
            .children()
            .filter(|child| child.type_projection().is_some())
            .collect::<Vec<_>>();
        match (paths.as_slice(), types.as_slice()) {
            ([path], [])
                if path.kind() == SyntaxKind::Path && path.role() == SyntaxRole::Target =>
            {
                path.path_projection()
                    .map(AttachedCandidatePathExpression::Value)
            }
            ([], [type_root]) if type_root.role() == SyntaxRole::Type => {
                let projection = type_root.type_projection()?;
                if !projection.path().steps().is_empty()
                    || projection.value().nominal_path().is_none()
                {
                    return None;
                }
                Some(AttachedCandidatePathExpression::NominalType(
                    AttachedCandidateNominalTypeRoot {
                        node: *type_root,
                        projection,
                    },
                ))
            }
            _ => None,
        }
    }
}
