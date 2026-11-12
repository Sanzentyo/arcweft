//! Borrowed, exact child projections of a retained operand.

use super::*;
use crate::types::constraints::shape::TypeConstraintChildren;

mod authority;
mod rebuild;
mod source_expectation;
pub(in crate::types::constraints) use rebuild::{
    ConstraintOperandDerivation, ConstraintOperandEffects, ConstraintOperandRebuild,
    ConstraintProjectedOperand,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConstraintOperandProjectionStep {
    TypeChild(usize),
    ArrayLength,
}

pub(super) enum ConstraintOperandProjection {
    Child {
        parent: Option<Arc<Self>>,
        index: usize,
    },
    ArrayLength {
        parent: Option<Arc<Self>>,
    },
}

#[cfg(test)]
impl ConstraintOperandProjection {
    pub(super) fn steps(&self) -> Vec<ConstraintOperandProjectionStep> {
        let mut steps = Vec::new();
        let mut current = Some(self);
        while let Some(address) = current {
            match address {
                Self::Child { parent, index } => {
                    steps.push(ConstraintOperandProjectionStep::TypeChild(*index));
                    current = parent.as_deref();
                }
                Self::ArrayLength { parent } => {
                    steps.push(ConstraintOperandProjectionStep::ArrayLength);
                    current = parent.as_deref();
                }
            }
        }
        steps.reverse();
        steps
    }
}

/// Child traversal borrows the shared type tree. Only a retained binding needs
/// an owned, transported value; its proof keeps the exact projection address.
pub(in crate::types) struct ConstraintOperandView<'a, D: ConstraintDomain> {
    root: &'a ConstraintOperand<D>,
    value: &'a TypeKind,
    scope: Arc<ConstraintLexicalScope>,
    address: Option<Arc<ConstraintOperandProjection>>,
    value_origin: Arc<ConstraintOperandOrigin<D>>,
}

impl<D: ConstraintDomain> Clone for ConstraintOperandView<'_, D> {
    fn clone(&self) -> Self {
        Self {
            root: self.root,
            value: self.value,
            scope: Arc::clone(&self.scope),
            address: self.address.clone(),
            value_origin: Arc::clone(&self.value_origin),
        }
    }
}

pub(in crate::types) struct ConstraintOperandChildren<'a, D: ConstraintDomain> {
    parent: ConstraintOperandView<'a, D>,
    children: std::iter::Enumerate<TypeConstraintChildren<'a>>,
    scope: Arc<ConstraintLexicalScope>,
}

impl<'a, D: ConstraintDomain> Iterator for ConstraintOperandChildren<'a, D> {
    type Item = Result<ConstraintOperandView<'a, D>, TypeConstraintError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.children.next().map(|(index, value)| {
            Ok(ConstraintOperandView {
                root: self.parent.root,
                value,
                scope: Arc::clone(&self.scope),
                address: Some(Arc::new(ConstraintOperandProjection::Child {
                    parent: self.parent.address.clone(),
                    index,
                })),
                value_origin: self.parent.value_origin.type_child_origin(index)?,
            })
        })
    }
}

pub(in crate::types) struct ConstraintConstOperandView<'a, D: ConstraintDomain> {
    origin: ConstraintConstOperandOrigin<'a, D>,
    value: &'a ArrayLength,
    scope: Arc<ConstraintLexicalScope>,
    value_origin: Arc<ConstraintOperandOrigin<D>>,
}

enum ConstraintConstOperandOrigin<'a, D: ConstraintDomain> {
    Array(ConstraintOperandView<'a, D>),
    Root(&'a ConstraintOperand<D, ArrayLength>),
}

impl<D: ConstraintDomain> Clone for ConstraintConstOperandView<'_, D> {
    fn clone(&self) -> Self {
        Self {
            origin: match &self.origin {
                ConstraintConstOperandOrigin::Array(parent) => {
                    ConstraintConstOperandOrigin::Array(parent.clone())
                }
                ConstraintConstOperandOrigin::Root(root) => {
                    ConstraintConstOperandOrigin::Root(root)
                }
            },
            value: self.value,
            scope: Arc::clone(&self.scope),
            value_origin: Arc::clone(&self.value_origin),
        }
    }
}

impl<'a, D: ConstraintDomain> ConstraintOperandView<'a, D> {
    pub(super) fn root(root: &'a ConstraintOperand<D>) -> Self {
        Self {
            root,
            value: root.value(),
            scope: Arc::clone(&root.scope),
            address: None,
            value_origin: Arc::clone(&root.origin),
        }
    }

    pub(in crate::types) fn value(&self) -> &'a TypeKind {
        self.value
    }

    pub(in crate::types) fn scope(&self) -> &crate::types::GenericScope {
        self.scope.shape()
    }

    pub(in crate::types::constraints) fn lexical_scope(&self) -> &Arc<ConstraintLexicalScope> {
        &self.scope
    }

    pub(in crate::types::constraints) fn retained(&self) -> ConstraintOperand<D> {
        match &self.address {
            None => self.root.clone(),
            Some(address) => ConstraintOperand {
                value: Arc::new(self.value.clone()),
                scope: Arc::clone(&self.scope),
                origin: self.projected_origin(Arc::clone(address), Arc::clone(&self.value_origin)),
            },
        }
    }

    pub(in crate::types) fn children<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintOperandChildren<'a, D>, TypeConstraintError> {
        let shape = self.value.constraint_shape();
        if !self
            .scope
            .admits_children_scope(context.lexical_frames(), shape.binder())
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        let scope = if shape.binder().is_empty() {
            Arc::clone(&self.scope)
        } else {
            Arc::new(context.capture_lexical_scope()?)
        };
        Ok(ConstraintOperandChildren {
            parent: self.clone(),
            children: shape.children().enumerate(),
            scope,
        })
    }

    pub(in crate::types) fn array_length(
        &self,
    ) -> Result<Option<ConstraintConstOperandView<'a, D>>, TypeConstraintError> {
        match self.value.constraint_shape() {
            crate::types::constraints::TypeConstraintShape::Array { len, .. } => {
                Ok(Some(ConstraintConstOperandView {
                    origin: ConstraintConstOperandOrigin::Array(self.clone()),
                    value: len,
                    scope: Arc::clone(&self.scope),
                    value_origin: self.value_origin.length_origin()?,
                }))
            }
            _ => Ok(None),
        }
    }

    fn projected_origin(
        &self,
        address: Arc<ConstraintOperandProjection>,
        selected: Arc<ConstraintOperandOrigin<D>>,
    ) -> Arc<ConstraintOperandOrigin<D>> {
        Arc::new(ConstraintOperandOrigin::Projection {
            root: Arc::clone(&self.root.value),
            root_scope: Arc::clone(&self.root.scope),
            occurrence_scope: Arc::clone(&self.scope),
            producer: Arc::clone(&self.root.origin),
            selected,
            address: Some(address),
        })
    }

    pub(in crate::types::constraints) fn transport<A: TypeConstraintAccounting>(
        &self,
        target: &Arc<ConstraintLexicalScope>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintOperand<D>, crate::types::TypeProjectionError<TypeConstraintError>> {
        let value = target.transport_type(&self.scope, self.value, context)?;
        let origin = match &self.address {
            Some(address) => {
                self.projected_origin(Arc::clone(address), Arc::clone(&self.value_origin))
            }
            None => Arc::clone(&self.root.origin),
        };
        Ok(ConstraintOperand {
            value: Arc::new(value),
            scope: Arc::clone(target),
            origin,
        })
    }
}

impl<'a, D: ConstraintDomain> ConstraintConstOperandView<'a, D> {
    pub(super) fn root(root: &'a ConstraintOperand<D, ArrayLength>) -> Self {
        Self {
            origin: ConstraintConstOperandOrigin::Root(root),
            value: root.value(),
            scope: Arc::clone(&root.scope),
            value_origin: Arc::clone(&root.origin),
        }
    }

    pub(in crate::types) fn value(&self) -> &'a ArrayLength {
        self.value
    }

    pub(in crate::types::constraints) fn lexical_scope(&self) -> &Arc<ConstraintLexicalScope> {
        &self.scope
    }

    pub(in crate::types::constraints) fn retained(&self) -> ConstraintOperand<D, ArrayLength> {
        match &self.origin {
            ConstraintConstOperandOrigin::Root(root) => (*root).clone(),
            ConstraintConstOperandOrigin::Array(parent) => ConstraintOperand {
                value: Arc::new(self.value.clone()),
                scope: Arc::clone(&self.scope),
                origin: parent.projected_origin(
                    Arc::new(ConstraintOperandProjection::ArrayLength {
                        parent: parent.address.clone(),
                    }),
                    Arc::clone(&self.value_origin),
                ),
            },
        }
    }

    pub(in crate::types::constraints) fn transport<A: TypeConstraintAccounting>(
        &self,
        target: &Arc<ConstraintLexicalScope>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        ConstraintOperand<D, ArrayLength>,
        crate::types::TypeProjectionError<TypeConstraintError>,
    > {
        let value = target.transport_length(&self.scope, self.value, context)?;
        let origin = match &self.origin {
            ConstraintConstOperandOrigin::Root(root) => Arc::clone(&root.origin),
            ConstraintConstOperandOrigin::Array(parent) => parent.projected_origin(
                Arc::new(ConstraintOperandProjection::ArrayLength {
                    parent: parent.address.clone(),
                }),
                Arc::clone(&self.value_origin),
            ),
        };
        Ok(ConstraintOperand {
            value: Arc::new(value),
            scope: Arc::clone(target),
            origin,
        })
    }
}
