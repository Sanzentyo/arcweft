//! Original lexical frames and metered transport into a joint inference scope.

use std::sync::Arc;

use super::{
    ConstraintDomain, TypeConstraintError,
    context::{TypeConstraintAccounting, TypeConstraintContext},
};
use crate::types::{
    ArrayLength, GenericBinder, GenericConstReference, GenericEffectReference, GenericScope,
    GenericTypeReference, TypeInstantiationError, TypeKind, TypeProjectionError,
};

struct LexicalFrame;

/// Shape alone is not provenance. Clones retain the actual opening of every
/// ambient frame; entering another equally shaped binder mints another frame.
#[derive(Clone, Default)]
pub(super) struct ConstraintLexicalScope {
    shape: GenericScope,
    frames: Vec<Arc<LexicalFrame>>,
}

impl PartialEq for ConstraintLexicalScope {
    fn eq(&self, other: &Self) -> bool {
        self.frames.len() == other.frames.len()
            && self
                .frames
                .iter()
                .zip(&other.frames)
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }
}

impl Eq for ConstraintLexicalScope {}

impl std::fmt::Debug for ConstraintLexicalScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConstraintLexicalScope")
            .field("shape", &self.shape)
            .finish_non_exhaustive()
    }
}

impl ConstraintLexicalScope {
    pub(super) fn transport_row<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        original: &Self,
        row: &crate::effect_row::EffectRow,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::EffectRow, TypeProjectionError<TypeConstraintError>> {
        super::solution::map_effects_with_control(row, 1, context, &|reference, context| {
            let reference = self.effect_reference(
                original,
                original.shape(),
                self.shape(),
                reference,
                context,
            )?;
            Ok(crate::effect_row::EffectRow::open(
                crate::effects::EffectSet::new(),
                reference,
            ))
        })
    }

    fn effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        original: &Self,
        source: &GenericScope,
        target: &GenericScope,
        reference: &GenericEffectReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericEffectReference, TypeProjectionError<TypeConstraintError>> {
        match reference {
            GenericEffectReference::Bound(bound) => {
                source.bound_effect(bound.depth(), bound.slot())?;
                Ok(target.bound_effect(
                    self.depth(original, source, bound.depth(), context)?,
                    bound.slot(),
                )?)
            }
            _ => Ok(reference.clone()),
        }
    }

    pub(super) fn transport_length<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        original: &Self,
        length: &ArrayLength,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeProjectionError<TypeConstraintError>> {
        context.enter_node().map_err(TypeProjectionError::Control)?;
        match length {
            ArrayLength::Generic(GenericConstReference::Bound(bound)) => {
                original.shape.bound_const(bound.depth(), bound.slot())?;
                Ok(ArrayLength::Generic(self.shape.bound_const(
                    self.depth(original, original.shape(), bound.depth(), context)?,
                    bound.slot(),
                )?))
            }
            ArrayLength::Generic(_) | ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        }
    }

    pub(super) fn transport_predicate<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        original: &Self,
        predicate: &crate::effect_row::EffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::EffectPredicate, TypeProjectionError<TypeConstraintError>> {
        super::solution::map_predicate_with_control(predicate, 1, context, &|reference, context| {
            let reference = self.effect_reference(
                original,
                original.shape(),
                self.shape(),
                reference,
                context,
            )?;
            Ok(crate::effect_row::EffectRow::open(
                crate::effects::EffectSet::new(),
                reference,
            ))
        })
    }

    pub(super) fn shape(&self) -> &GenericScope {
        &self.shape
    }

    pub(super) fn with_binder(&self, binder: GenericBinder) -> Self {
        let mut scope = self.clone();
        if !binder.is_empty() {
            scope.shape = self.shape.with_binder(binder);
            scope.frames.push(Arc::new(LexicalFrame));
        }
        scope
    }

    /// A type's children use these same actual frames, with only its own
    /// function binder added by the live relation traversal.
    pub(super) fn admits_children_scope(&self, target: &Self, binder: GenericBinder) -> bool {
        if binder.is_empty() {
            return self == target;
        }
        self.frames.len().checked_add(1) == Some(target.frames.len())
            && target.shape.binders().last() == Some(&binder)
            && self
                .frames
                .iter()
                .zip(&target.frames)
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }

    /// A template traversal is isolated from the actual operands. Its arities
    /// cannot certify an alignment with an independently retained actual scope.
    pub(super) fn template(shape: GenericScope) -> Self {
        let frames = shape
            .binders()
            .iter()
            .map(|_| Arc::new(LexicalFrame))
            .collect();
        Self { shape, frames }
    }

    pub(super) fn merge<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &mut self,
        original: &Self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        for (frame, binder) in original.frames.iter().zip(original.shape.binders()) {
            context.enter_node()?;
            let mut found = false;
            for existing in &self.frames {
                context.enter_node()?;
                if Arc::ptr_eq(existing, frame) {
                    found = true;
                    break;
                }
            }
            if !found {
                for _ in &self.frames {
                    context.enter_node()?;
                }
                context.enter_node()?;
                self.shape = self.shape.with_binder(*binder);
                self.frames.push(Arc::clone(frame));
            }
        }
        Ok(())
    }

    fn depth<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        original: &Self,
        occurrence: &GenericScope,
        depth: u32,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<u32, TypeProjectionError<TypeConstraintError>> {
        let nested = occurrence
            .binders()
            .len()
            .checked_sub(original.frames.len())
            .ok_or(TypeInstantiationError::SpecializationScopeMismatch)?;
        let depth_index =
            usize::try_from(depth).map_err(|_| TypeInstantiationError::DepthOverflow)?;
        if depth_index < nested {
            return Ok(depth);
        }
        self.ambient_depth(original, depth, nested, nested, context)
    }

    /// Move an ambient reference while independently inserting/removing local
    /// quantifiers. Residual slots and function-owned slots are handled by the
    /// scoped reference mapper before entering this actual-frame operation.
    pub(super) fn ambient_depth<C: crate::types::TypeProjectionControl>(
        &self,
        original: &Self,
        depth: u32,
        source_inner: usize,
        target_inner: usize,
        control: &mut C,
    ) -> Result<u32, TypeProjectionError<C::Error>> {
        let depth_index =
            usize::try_from(depth).map_err(|_| TypeInstantiationError::DepthOverflow)?;
        let distance = depth_index
            .checked_sub(source_inner)
            .ok_or(TypeInstantiationError::SpecializationScopeMismatch)?
            .checked_add(1)
            .ok_or(TypeInstantiationError::DepthOverflow)?;
        let index = original
            .frames
            .len()
            .checked_sub(distance)
            .ok_or(TypeInstantiationError::SpecializationScopeMismatch)?;
        let frame = &original.frames[index];
        for (target, existing) in self.frames.iter().enumerate() {
            control
                .visit_node(crate::types::TypeProjectionNodeKind::Type, 1)
                .map_err(TypeProjectionError::Control)?;
            if Arc::ptr_eq(existing, frame) {
                let depth = (self.frames.len() - target - 1)
                    .checked_add(target_inner)
                    .ok_or(TypeInstantiationError::DepthOverflow)?;
                return u32::try_from(depth)
                    .map_err(|_| TypeInstantiationError::DepthOverflow.into());
            }
        }
        Err(TypeInstantiationError::SpecializationScopeMismatch.into())
    }

    pub(super) fn transport_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        original: &Self,
        ty: &TypeKind,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeProjectionError<TypeConstraintError>> {
        super::solution::map_term_with_control(
            ty,
            original.shape(),
            self.shape(),
            1,
            context,
            &|reference, source, target, _, context| {
                Ok(TypeKind::GenericParam(match reference {
                    GenericTypeReference::Bound(bound) => {
                        source.bound_type(bound.depth(), bound.slot())?;
                        target.bound_type(
                            self.depth(original, source, bound.depth(), context)?,
                            bound.slot(),
                        )?
                    }
                    _ => reference.clone(),
                }))
            },
            &|length, source, target, _, context| match length {
                ArrayLength::Generic(GenericConstReference::Bound(bound)) => {
                    source.bound_const(bound.depth(), bound.slot())?;
                    Ok(ArrayLength::Generic(target.bound_const(
                        self.depth(original, source, bound.depth(), context)?,
                        bound.slot(),
                    )?))
                }
                ArrayLength::Generic(_) | ArrayLength::Const(_) => Ok(length.clone()),
                ArrayLength::Error(_) | ArrayLength::Inferred => {
                    Err(TypeInstantiationError::UnresolvedType.into())
                }
            },
            &|row, source, target, depth, context| {
                super::solution::map_effects_with_control(
                    row,
                    depth,
                    context,
                    &|reference, context| {
                        let reference =
                            self.effect_reference(original, source, target, reference, context)?;
                        Ok(crate::effect_row::EffectRow::open(
                            crate::effects::EffectSet::new(),
                            reference,
                        ))
                    },
                )
            },
        )
    }
}
