//! Flat specialization of a completed application in its caller's environment.
//!
//! Binding right-hand sides belong to the caller. They are projected exactly
//! once through the enclosing environment, never through the callee's keys.
//! The resulting rows are the callee's environment for its declaration body.

use thiserror::Error;

use crate::effect_row::{EffectRow, EffectRowError, EffectVar};
use crate::types::{
    ArrayLength, GenericBinder, GenericConstReference, GenericParameterKind, GenericScope,
    GenericScopeError, GenericTypeReference, ScopedArrayLengthView, ScopedConstReferenceView,
    ScopedTypeReferenceView, ScopedTypeView, TypeKind, TypeProjectionControl, TypeProjectionError,
    TypeProjectionNodeKind,
};

use crate::types::projection_control::{UnmeteredTypeProjection, visit_effect_row};

use super::{
    CheckedConstArgumentBinding, CheckedEffectArgumentBinding, CheckedTypeArgumentBinding,
    TypeConstraintSolution,
};

/// Failure to specialize a semantic type in a closed declaration environment.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TypeInstantiationError {
    #[error(transparent)]
    Scope(#[from] GenericScopeError),
    #[error(transparent)]
    Effect(#[from] EffectRowError),
    #[error("type parameter has no closed instance binding: {parameter:?}")]
    UnboundType { parameter: GenericTypeReference },
    #[error("constant parameter has no closed instance binding: {parameter:?}")]
    UnboundConst { parameter: GenericConstReference },
    #[error("application still owns residual generic quantifiers: {binder:?}")]
    Residual { binder: GenericBinder },
    #[error("unresolved semantic type reached closed instance projection")]
    UnresolvedType,
    #[error("semantic type projection depth overflowed")]
    DepthOverflow,
    #[error("canonical semantic encoding length exceeds u64")]
    EncodingLengthOverflow,
}

/// A closed, simultaneous substitution. It contains no enclosing environments
/// or application history. Construction is confined to completion below.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ClosedTypeInstantiation {
    template_scope: GenericScope,
    bindings: Box<[CheckedTypeArgumentBinding]>,
    const_bindings: Box<[CheckedConstArgumentBinding]>,
    effect_bindings: Box<[CheckedEffectArgumentBinding]>,
}

impl TypeConstraintSolution {
    #[cfg(test)]
    pub(crate) fn close_instantiation(
        &self,
        enclosing: Option<&ClosedTypeInstantiation>,
    ) -> Result<ClosedTypeInstantiation, TypeInstantiationError> {
        self.close_instantiation_with_control(enclosing, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn close_instantiation_with_control<C: TypeProjectionControl>(
        &self,
        enclosing: Option<&ClosedTypeInstantiation>,
        control: &mut C,
    ) -> Result<ClosedTypeInstantiation, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        if !self.residual.binder().is_empty() {
            return Err(TypeInstantiationError::Residual {
                binder: self.residual.binder(),
            }
            .into());
        }
        let empty = ClosedTypeInstantiation::default();
        let caller = enclosing.unwrap_or(&empty);
        let bindings = self
            .bindings()
            .map(|(parameter, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                // Effect variables have their own issuer namespace. Resolve
                // the completed application's rows before caller closure;
                // type/const declaration keys are never reapplied here.
                Ok(CheckedTypeArgumentBinding::new(
                    parameter.value().clone(),
                    caller.project_type_with_control(
                        value.value(),
                        &GenericScope::default(),
                        Some(self),
                        control,
                    )?,
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let const_bindings = self
            .const_bindings()
            .map(|(parameter, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                control
                    .visit_node(TypeProjectionNodeKind::Const, 1)
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedConstArgumentBinding::new(
                    parameter.value().clone(),
                    caller.project_length_with_control(
                        value.value(),
                        value.scope(),
                        value.scope(),
                        1,
                        control,
                    )?,
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let effect_bindings = self
            .effect_bindings()
            .map(|(variable, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedEffectArgumentBinding::new(
                    *variable,
                    EffectRow::closed(caller.project_effect_row_with_control(value, 1, control)?),
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        Ok(ClosedTypeInstantiation {
            template_scope: self.authority.parameter_scope.template_scope().clone(),
            bindings,
            const_bindings,
            effect_bindings,
        })
    }
}

impl ClosedTypeInstantiation {
    pub(crate) fn type_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (ScopedTypeReferenceView<'_>, ScopedTypeView<'_>)> {
        self.bindings.iter().map(|row| {
            (
                ScopedTypeReferenceView::sealed(&row.parameter, &self.template_scope),
                ScopedTypeView::at_root(&row.value),
            )
        })
    }

    pub(crate) fn const_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (ScopedConstReferenceView<'_>, ScopedArrayLengthView<'_>)>
    {
        self.const_bindings.iter().map(|row| {
            (
                ScopedConstReferenceView::sealed(&row.parameter, &self.template_scope),
                ScopedArrayLengthView::at_root(&row.value),
            )
        })
    }

    pub(crate) fn effect_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (&crate::effect_row::EffectVar, &EffectRow)> {
        self.effect_bindings
            .iter()
            .map(|row| (&row.variable, &row.value))
    }

    pub(crate) fn instantiate_type(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, TypeInstantiationError> {
        self.instantiate_type_with_control(ty, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn instantiate_type_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        self.project_type_with_control(ty, &self.template_scope, None, control)
    }

    pub(crate) fn instantiate_array_length(
        &self,
        length: &ArrayLength,
    ) -> Result<ArrayLength, TypeInstantiationError> {
        self.instantiate_array_length_with_control(length, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn instantiate_array_length_with_control<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        control
            .visit_node(TypeProjectionNodeKind::Const, 1)
            .map_err(TypeProjectionError::Control)?;
        self.project_length_with_control(
            length,
            &self.template_scope,
            &self.template_scope,
            1,
            control,
        )
    }

    pub(crate) fn instantiate_effect_row(
        &self,
        row: &EffectRow,
    ) -> Result<crate::effects::EffectSet, EffectRowError> {
        row.resolve_with(|variable| self.effect_binding(variable), |_| Ok(()))
    }

    fn effect_binding(&self, variable: EffectVar) -> Option<&EffectRow> {
        self.effect_bindings
            .binary_search_by_key(&variable, |row| row.variable)
            .ok()
            .map(|index| &self.effect_bindings[index].value)
    }

    pub(crate) fn project_effect_row_with_control<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        depth: u64,
        control: &mut C,
    ) -> Result<crate::effects::EffectSet, TypeProjectionError<C::Error>> {
        row.resolve_with(
            |variable| self.effect_binding(variable),
            |row| visit_effect_row(control, row, depth),
        )
    }

    fn project_type_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        incoming: &GenericScope,
        effect_overlay: Option<&TypeConstraintSolution>,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        super::template::map_term_with_control(
            ty,
            incoming,
            &GenericScope::default(),
            1,
            control,
            &|reference, scope, _, depth, control| {
                if let Some(parameter) = reference.template_key(incoming, scope)? {
                    let index = self
                        .bindings
                        .binary_search_by(|row| row.parameter.cmp(&parameter))
                        .map_err(|_| TypeInstantiationError::UnboundType { parameter })?;
                    return super::template::clone_term_with_control(
                        &self.bindings[index].value,
                        depth,
                        control,
                    );
                }
                match reference {
                    GenericTypeReference::Free(_) => {
                        unreachable!("free references have template keys")
                    }
                    GenericTypeReference::Bound(parameter) => Ok(TypeKind::GenericParam(
                        scope.bound_type(parameter.depth(), parameter.slot())?,
                    )),
                    GenericTypeReference::Inference(_) => {
                        Err(GenericScopeError::EscapedInference {
                            kind: GenericParameterKind::Type,
                        }
                        .into())
                    }
                }
            },
            &|length, scope, _, depth, control| {
                self.project_length_with_control(length, incoming, scope, depth, control)
            },
            &|row, depth, control| {
                let projected = match effect_overlay {
                    Some(overlay) => {
                        let row = row.resolve_partial_with(
                            |variable| {
                                overlay
                                    .effect_bindings
                                    .binary_search_by_key(&variable, |row| row.variable)
                                    .ok()
                                    .map(|index| &overlay.effect_bindings[index].value)
                            },
                            |row| visit_effect_row(control, row, depth),
                        )?;
                        self.project_effect_row_with_control(&row, depth, control)?
                    }
                    None => self.project_effect_row_with_control(row, depth, control)?,
                };
                Ok(EffectRow::closed(projected))
            },
        )
    }

    fn project_length_with_control<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        incoming: &GenericScope,
        scope: &GenericScope,
        depth: u64,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        match length {
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Generic(reference) => {
                if let Some(parameter) = reference.template_key(incoming, scope)? {
                    let index = self
                        .const_bindings
                        .binary_search_by(|row| row.parameter.cmp(&parameter))
                        .map_err(|_| TypeInstantiationError::UnboundConst { parameter })?;
                    control
                        .visit_node(TypeProjectionNodeKind::Const, depth)
                        .map_err(TypeProjectionError::Control)?;
                    return Ok(self.const_bindings[index].value.clone());
                }
                Ok(length.clone())
            }
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        }
    }
}
