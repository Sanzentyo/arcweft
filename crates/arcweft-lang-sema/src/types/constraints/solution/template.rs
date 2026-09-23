//! Capture-avoiding application of completed rows to declaration templates.

use crate::effect_row::EffectRow;
use crate::types::{
    ArrayLength, GenericBinder, GenericConstReference, GenericEffectReference,
    GenericParameterKind, GenericScope, GenericScopeError, GenericTypeReference, ScopedType,
    ScopedTypeView, TypeKind, TypeProjectionControl, TypeProjectionError, TypeProjectionNodeKind,
};

use super::super::shape::{TypeConstraintChildren, TypeConstraintShape};
use crate::types::projection_control::visit_effect_row;
use crate::types::projection_control::{EffectProjectionControl, UnmeteredTypeProjection};

use super::{TypeConstraintSolution, TypeInstantiationError};

impl TypeConstraintSolution {
    /// Template references name this solution's formal parameters. Replacement
    /// values are caller-owned: their free declarations are never looked up in
    /// this solution again, including same-declaration recursive applications.
    pub(crate) fn apply_template(
        &self,
        ty: &TypeKind,
    ) -> Result<ScopedType, TypeInstantiationError> {
        let residual = self.residual.scope();
        let template = self.authority.parameter_scope.template_scope();
        let value = map_term(
            ty,
            template,
            residual,
            &|reference, source, target| {
                if let Some(parameter) = reference.template_key(template, source)? {
                    if let Ok(index) = self
                        .bindings
                        .binary_search_by(|row| row.parameter.cmp(&parameter))
                    {
                        let value = ScopedTypeView::sealed(&self.bindings[index].value, residual);
                        return lift_value(value, target);
                    }
                    if let Some(slot) = self.residual.type_slot(&parameter) {
                        return Ok(TypeKind::GenericParam(
                            target.bound_type(depth_difference(target, residual)?, slot)?,
                        ));
                    }
                    if matches!(parameter, GenericTypeReference::Bound(_)) {
                        return Err(TypeInstantiationError::UnboundType { parameter });
                    }
                }
                keep_type(reference, source)
            },
            &|reference, source, target| {
                if let Some(parameter) = reference.template_key(template, source)? {
                    if let Ok(index) = self
                        .const_bindings
                        .binary_search_by(|row| row.parameter.cmp(&parameter))
                    {
                        return lift_length(&self.const_bindings[index].value, residual, target);
                    }
                    if let Some(slot) = self.residual.const_slot(&parameter) {
                        return Ok(ArrayLength::Generic(
                            target.bound_const(depth_difference(target, residual)?, slot)?,
                        ));
                    }
                    if matches!(parameter, GenericConstReference::Bound(_)) {
                        return Err(TypeInstantiationError::UnboundConst { parameter });
                    }
                }
                keep_const(reference, source)
            },
            &|row, source, target| {
                map_effects(row, &|reference| {
                    if let Some(parameter) = reference.template_key(template, source)? {
                        if let Ok(index) = self
                            .effect_bindings
                            .binary_search_by(|row| row.variable.cmp(&parameter))
                        {
                            return lift_effects(
                                &self.effect_bindings[index].value,
                                residual,
                                target,
                            );
                        }
                        if let Some(slot) = self.residual.effect_slot(&parameter) {
                            return Ok(EffectRow::open(
                                crate::effects::EffectSet::new(),
                                target.bound_effect(depth_difference(target, residual)?, slot)?,
                            ));
                        }
                        if matches!(parameter, GenericEffectReference::Bound(_)) {
                            return Err(TypeInstantiationError::UnboundEffect { parameter });
                        }
                    }
                    Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        keep_effect(reference, source)?,
                    ))
                })
            },
        )?;
        Ok(ScopedType::new(value, residual.clone()))
    }
}

impl ScopedTypeView<'_> {
    /// Closes a value projection without granting it the solution's residual
    /// quantifiers. Unused incoming scope can disappear only after this walk
    /// proves every retained bound reference has an owner in the value itself.
    pub(crate) fn to_root_type(self) -> Result<TypeKind, TypeInstantiationError> {
        let root = GenericScope::default();
        map_term(
            self.value(),
            &root,
            &root,
            &|reference, source, _| keep_type(reference, source),
            &|reference, source, _| keep_const(reference, source),
            &|row, source, _| {
                map_effects(row, &|reference| {
                    Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        keep_effect(reference, source)?,
                    ))
                })
            },
        )
    }

    /// Transfers incoming quantifiers to the returned function itself. All
    /// existing function-local binders are preserved, with depth and slot
    /// remapping when the incoming and root function binders are combined.
    pub(crate) fn to_quantified_type(self) -> Result<TypeKind, TypeInstantiationError> {
        if self.scope().binders().is_empty() {
            return self.to_root_type();
        }
        let TypeKind::Function {
            binder,
            params,
            return_type,
            effects,
        } = self.value()
        else {
            return Err(TypeInstantiationError::Residual {
                binder: merge_binders(self.scope().binders())?,
            });
        };
        let source = self.scope().with_binder(*binder);
        let merged = merge_binders(source.binders())?;
        let target = GenericScope::default().with_binder(merged);
        let root_depth = source.binders().len();
        let type_map =
            |reference: &GenericTypeReference, source: &GenericScope, target: &GenericScope| {
                let GenericTypeReference::Bound(parameter) = reference else {
                    return keep_type(reference, source);
                };
                source.bound_type(parameter.depth(), parameter.slot())?;
                let (depth, slot) = fused_coordinate(
                    source,
                    target,
                    root_depth,
                    parameter.depth(),
                    parameter.slot(),
                    GenericParameterKind::Type,
                )?;
                Ok(TypeKind::GenericParam(target.bound_type(depth, slot)?))
            };
        let const_map =
            |reference: &GenericConstReference, source: &GenericScope, target: &GenericScope| {
                let GenericConstReference::Bound(parameter) = reference else {
                    return keep_const(reference, source);
                };
                source.bound_const(parameter.depth(), parameter.slot())?;
                let (depth, slot) = fused_coordinate(
                    source,
                    target,
                    root_depth,
                    parameter.depth(),
                    parameter.slot(),
                    GenericParameterKind::Const,
                )?;
                Ok(ArrayLength::Generic(target.bound_const(depth, slot)?))
            };
        let effect_map = |row: &EffectRow, source: &GenericScope, target: &GenericScope| {
            map_effects(row, &|reference| {
                let GenericEffectReference::Bound(parameter) = reference else {
                    return Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        keep_effect(reference, source)?,
                    ));
                };
                source.bound_effect(parameter.depth(), parameter.slot())?;
                let local = source.binders().len() - root_depth;
                let (depth, slot) = if (parameter.depth() as usize) < local {
                    (parameter.depth(), parameter.slot())
                } else {
                    let owner = source.binders().len() - 1 - parameter.depth() as usize;
                    let offset = merge_binders(&source.binders()[..owner])?.effects();
                    let slot = parameter.slot().checked_add(offset).ok_or(
                        GenericScopeError::BinderArityOverflow {
                            kind: GenericParameterKind::Effect,
                            count: parameter.slot() as usize + offset as usize,
                        },
                    )?;
                    (
                        u32::try_from(target.binders().len() - 1).map_err(|_| {
                            GenericScopeError::UnknownDepth {
                                depth: parameter.depth(),
                            }
                        })?,
                        slot,
                    )
                };
                Ok(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    target.bound_effect(depth, slot)?,
                ))
            })
        };
        let project =
            |ty: &TypeKind| map_term(ty, &source, &target, &type_map, &const_map, &effect_map);
        Ok(TypeKind::function_with_binder(
            merged,
            params.iter().map(project).collect::<Result<Vec<_>, _>>()?,
            project(return_type)?,
            effect_map(effects, &source, &target)?,
        ))
    }
}

fn merge_binders(binders: &[GenericBinder]) -> Result<GenericBinder, TypeInstantiationError> {
    binders
        .iter()
        .try_fold(GenericBinder::EMPTY, |combined, next| {
            combined.checked_append(*next).map_err(Into::into)
        })
}
fn fused_coordinate(
    source: &GenericScope,
    target: &GenericScope,
    root_depth: usize,
    depth: u32,
    slot: u16,
    kind: GenericParameterKind,
) -> Result<(u32, u16), TypeInstantiationError> {
    let local = source.binders().len() - root_depth;
    if (depth as usize) < local {
        return Ok((depth, slot));
    }
    let owner = source.binders().len() - 1 - depth as usize;
    let prefix = merge_binders(&source.binders()[..owner])?;
    let offset = match kind {
        GenericParameterKind::Type => prefix.types(),
        GenericParameterKind::Const => prefix.const_lengths(),
        GenericParameterKind::Effect => unreachable!("type and const coordinates use u16 slots"),
    };
    let slot = slot
        .checked_add(offset)
        .ok_or(GenericScopeError::BinderArityOverflow {
            kind,
            count: usize::from(slot) + usize::from(offset),
        })?;
    Ok((
        u32::try_from(target.binders().len() - 1)
            .map_err(|_| GenericScopeError::UnknownDepth { depth })?,
        slot,
    ))
}

fn depth_difference(
    target: &GenericScope,
    source: &GenericScope,
) -> Result<u32, TypeInstantiationError> {
    target
        .binders()
        .len()
        .checked_sub(source.binders().len())
        .and_then(|depth| u32::try_from(depth).ok())
        .ok_or_else(|| GenericScopeError::UnknownDepth { depth: u32::MAX }.into())
}

fn lift_value(
    value: ScopedTypeView<'_>,
    target: &GenericScope,
) -> Result<TypeKind, TypeInstantiationError> {
    let inserted = depth_difference(target, value.scope())?;
    let root_depth = value.scope().binders().len();
    map_term(
        value.value(),
        value.scope(),
        target,
        &|reference, source, target| {
            let GenericTypeReference::Bound(parameter) = reference else {
                return keep_type(reference, source);
            };
            source.bound_type(parameter.depth(), parameter.slot())?;
            let local = source.binders().len() - root_depth;
            let depth = if (parameter.depth() as usize) < local {
                parameter.depth()
            } else {
                parameter
                    .depth()
                    .checked_add(inserted)
                    .ok_or(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    })?
            };
            Ok(TypeKind::GenericParam(
                target.bound_type(depth, parameter.slot())?,
            ))
        },
        &|reference, source, target| {
            let GenericConstReference::Bound(parameter) = reference else {
                return keep_const(reference, source);
            };
            source.bound_const(parameter.depth(), parameter.slot())?;
            let local = source.binders().len() - root_depth;
            let depth = if (parameter.depth() as usize) < local {
                parameter.depth()
            } else {
                parameter
                    .depth()
                    .checked_add(inserted)
                    .ok_or(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    })?
            };
            Ok(ArrayLength::Generic(
                target.bound_const(depth, parameter.slot())?,
            ))
        },
        &|row, source, target| {
            map_effects(row, &|reference| {
                let reference = lift_effect(reference, source, target, root_depth, inserted)?;
                Ok(EffectRow::open(crate::effects::EffectSet::new(), reference))
            })
        },
    )
}

fn lift_length(
    length: &ArrayLength,
    source: &GenericScope,
    target: &GenericScope,
) -> Result<ArrayLength, TypeInstantiationError> {
    match length {
        ArrayLength::Generic(GenericConstReference::Bound(parameter)) => {
            source.bound_const(parameter.depth(), parameter.slot())?;
            let depth = parameter
                .depth()
                .checked_add(depth_difference(target, source)?)
                .ok_or(GenericScopeError::UnknownDepth {
                    depth: parameter.depth(),
                })?;
            Ok(ArrayLength::Generic(
                target.bound_const(depth, parameter.slot())?,
            ))
        }
        ArrayLength::Generic(reference) => keep_const(reference, source),
        ArrayLength::Const(_) => Ok(length.clone()),
        ArrayLength::Error(_) | ArrayLength::Inferred => {
            Err(TypeInstantiationError::UnresolvedType)
        }
    }
}

fn keep_type(
    reference: &GenericTypeReference,
    scope: &GenericScope,
) -> Result<TypeKind, TypeInstantiationError> {
    match reference {
        GenericTypeReference::Free(_) => Ok(TypeKind::GenericParam(reference.clone())),
        GenericTypeReference::Bound(parameter) => Ok(TypeKind::GenericParam(
            scope.bound_type(parameter.depth(), parameter.slot())?,
        )),
        GenericTypeReference::Inference(_) => Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Type,
        }
        .into()),
    }
}

fn keep_const(
    reference: &GenericConstReference,
    scope: &GenericScope,
) -> Result<ArrayLength, TypeInstantiationError> {
    match reference {
        GenericConstReference::Free(_) => Ok(ArrayLength::Generic(reference.clone())),
        GenericConstReference::Bound(parameter) => Ok(ArrayLength::Generic(
            scope.bound_const(parameter.depth(), parameter.slot())?,
        )),
        GenericConstReference::Inference(_) => Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Const,
        }
        .into()),
    }
}

pub(super) fn keep_effect(
    reference: &GenericEffectReference,
    scope: &GenericScope,
) -> Result<GenericEffectReference, TypeInstantiationError> {
    match reference {
        GenericEffectReference::Free(_) => Ok(reference.clone()),
        GenericEffectReference::Bound(parameter) => {
            Ok(scope.bound_effect(parameter.depth(), parameter.slot())?)
        }
        GenericEffectReference::Inference(_) => Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Effect,
        }
        .into()),
    }
}

fn lift_effect(
    reference: &GenericEffectReference,
    source: &GenericScope,
    target: &GenericScope,
    root_depth: usize,
    inserted: u32,
) -> Result<GenericEffectReference, TypeInstantiationError> {
    let GenericEffectReference::Bound(parameter) = reference else {
        return keep_effect(reference, source);
    };
    source.bound_effect(parameter.depth(), parameter.slot())?;
    let local = source.binders().len() - root_depth;
    let depth = if (parameter.depth() as usize) < local {
        parameter.depth()
    } else {
        parameter
            .depth()
            .checked_add(inserted)
            .ok_or(GenericScopeError::UnknownDepth {
                depth: parameter.depth(),
            })?
    };
    Ok(target.bound_effect(depth, parameter.slot())?)
}

fn lift_effects(
    row: &EffectRow,
    source: &GenericScope,
    target: &GenericScope,
) -> Result<EffectRow, TypeInstantiationError> {
    let inserted = depth_difference(target, source)?;
    map_effects(row, &|reference| {
        Ok(EffectRow::open(
            crate::effects::EffectSet::new(),
            lift_effect(reference, source, target, source.binders().len(), inserted)?,
        ))
    })
}

fn map_effects(
    row: &EffectRow,
    mapping: &impl Fn(&GenericEffectReference) -> Result<EffectRow, TypeInstantiationError>,
) -> Result<EffectRow, TypeInstantiationError> {
    map_effects_with_control(row, 1, &mut UnmeteredTypeProjection, &|reference, _| {
        mapping(reference).map_err(Into::into)
    })
    .map_err(TypeProjectionError::into_instantiation)
}

pub(super) fn map_effects_with_control<C: TypeProjectionControl>(
    row: &EffectRow,
    depth: u64,
    control: &mut C,
    mapping: &impl Fn(
        &GenericEffectReference,
        &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
) -> Result<EffectRow, TypeProjectionError<C::Error>> {
    visit_effect_row(control, row, depth)?;
    row.try_substitute_variables(
        &mut EffectProjectionControl { control, depth },
        &mut |reference, adapter| mapping(reference, adapter.control),
    )
}

pub(super) fn map_term(
    ty: &TypeKind,
    source: &GenericScope,
    target: &GenericScope,
    types: &impl Fn(
        &GenericTypeReference,
        &GenericScope,
        &GenericScope,
    ) -> Result<TypeKind, TypeInstantiationError>,
    consts: &impl Fn(
        &GenericConstReference,
        &GenericScope,
        &GenericScope,
    ) -> Result<ArrayLength, TypeInstantiationError>,
    effects: &impl Fn(
        &EffectRow,
        &GenericScope,
        &GenericScope,
    ) -> Result<EffectRow, TypeInstantiationError>,
) -> Result<TypeKind, TypeInstantiationError> {
    map_term_with_control(
        ty,
        source,
        target,
        1,
        &mut UnmeteredTypeProjection,
        &|reference, source, target, _, _| types(reference, source, target).map_err(Into::into),
        &|length, source, target, _, _| match length {
            ArrayLength::Generic(reference) => {
                consts(reference, source, target).map_err(Into::into)
            }
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        },
        &|row, source, target, _, _| effects(row, source, target).map_err(Into::into),
    )
    .map_err(TypeProjectionError::into_instantiation)
}

struct ProjectionFrame<'ty> {
    shape: TypeConstraintShape<'ty>,
    source: GenericScope,
    target: GenericScope,
    depth: u64,
    children: TypeConstraintChildren<'ty>,
    projected: Vec<TypeKind>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "one scoped fold owns source and target binders, accounting, and the three scalar projection operations"
)]
pub(super) fn map_term_with_control<C: TypeProjectionControl>(
    ty: &TypeKind,
    source: &GenericScope,
    target: &GenericScope,
    depth: u64,
    control: &mut C,
    types: &impl Fn(
        &GenericTypeReference,
        &GenericScope,
        &GenericScope,
        u64,
        &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>>,
    consts: &impl Fn(
        &ArrayLength,
        &GenericScope,
        &GenericScope,
        u64,
        &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>>,
    effects: &impl Fn(
        &EffectRow,
        &GenericScope,
        &GenericScope,
        u64,
        &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
) -> Result<TypeKind, TypeProjectionError<C::Error>> {
    control.check().map_err(TypeProjectionError::Control)?;
    let mut frames = Vec::<ProjectionFrame<'_>>::new();
    let mut current = (ty, source.clone(), target.clone(), depth);
    loop {
        let (ty, source, target, depth) = current;
        control.check().map_err(TypeProjectionError::Control)?;
        control
            .visit_node(TypeProjectionNodeKind::Type, depth)
            .map_err(TypeProjectionError::Control)?;
        let mut value = if let TypeKind::GenericParam(reference) = ty {
            types(reference, &source, &target, depth, control)?
        } else {
            let shape = ty.constraint_shape();
            let mut frame = ProjectionFrame {
                shape,
                source: source.with_binder(shape.binder()),
                target: target.with_binder(shape.binder()),
                depth,
                children: shape.children(),
                projected: Vec::new(),
            };
            if let Some(child) = frame.children.next() {
                current = (
                    child,
                    frame.source.clone(),
                    frame.target.clone(),
                    child_depth(depth)?,
                );
                frames.push(frame);
                continue;
            }
            frame.finish(control, consts, effects)?
        };
        loop {
            let Some(mut parent) = frames.pop() else {
                return Ok(value);
            };
            parent.projected.push(value);
            if let Some(child) = parent.children.next() {
                current = (
                    child,
                    parent.source.clone(),
                    parent.target.clone(),
                    child_depth(parent.depth)?,
                );
                frames.push(parent);
                break;
            }
            value = parent.finish(control, consts, effects)?;
        }
    }
}

fn child_depth<E: std::error::Error + 'static>(depth: u64) -> Result<u64, TypeProjectionError<E>> {
    depth
        .checked_add(1)
        .ok_or_else(|| TypeInstantiationError::DepthOverflow.into())
}

impl ProjectionFrame<'_> {
    fn finish<C: TypeProjectionControl>(
        self,
        control: &mut C,
        consts: &impl Fn(
            &ArrayLength,
            &GenericScope,
            &GenericScope,
            u64,
            &mut C,
        ) -> Result<ArrayLength, TypeProjectionError<C::Error>>,
        effects: &impl Fn(
            &EffectRow,
            &GenericScope,
            &GenericScope,
            u64,
            &mut C,
        ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        self.shape.rebuild_with(
            self.projected,
            control,
            |control, length| {
                let depth = child_depth(self.depth)?;
                control.check().map_err(TypeProjectionError::Control)?;
                control
                    .visit_node(TypeProjectionNodeKind::Const, depth)
                    .map_err(TypeProjectionError::Control)?;
                consts(length, &self.source, &self.target, depth, control)
            },
            |control, row| {
                effects(
                    row,
                    &self.source,
                    &self.target,
                    child_depth(self.depth)?,
                    control,
                )
            },
        )
    }
}

/// Copies a rooted replacement without applying the callee's keys to it.
/// Every descendant is visited before copying; nested function binders keep
/// their own lexical coordinates.
pub(super) fn clone_term_with_control<C: TypeProjectionControl>(
    ty: &TypeKind,
    depth: u64,
    control: &mut C,
) -> Result<TypeKind, TypeProjectionError<C::Error>> {
    let root = GenericScope::default();
    map_term_with_control(
        ty,
        &root,
        &root,
        depth,
        control,
        &|reference, scope, _, _, _| keep_type(reference, scope).map_err(Into::into),
        &|length, scope, _, _, _| match length {
            ArrayLength::Generic(reference) => keep_const(reference, scope).map_err(Into::into),
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        },
        &|row, source, _, depth, control| {
            map_effects_with_control(row, depth, control, &|reference, _| {
                Ok(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    keep_effect(reference, source)?,
                ))
            })
        },
    )
}

impl TypeKind {
    /// Instantiates declaration parameters simultaneously, charging every
    /// occurrence before copying it. Replacements retain their own scope and
    /// are never substituted through the declaration a second time.
    pub(crate) fn instantiate_type_parameters_with_control<C: TypeProjectionControl>(
        &self,
        substitutions: &std::collections::BTreeMap<
            &crate::types::GenericTypeParameterId,
            &TypeKind,
        >,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let root = GenericScope::default();
        map_term_with_control(
            self,
            &root,
            &root,
            1,
            control,
            &|reference, scope, _, depth, control| match reference {
                GenericTypeReference::Free(parameter) => {
                    control
                        .visit_binding()
                        .map_err(TypeProjectionError::Control)?;
                    let replacement = substitutions.get(parameter).ok_or_else(|| {
                        TypeInstantiationError::UnboundType {
                            parameter: reference.clone(),
                        }
                    })?;
                    clone_term_with_control(replacement, depth, control)
                }
                _ => keep_type(reference, scope).map_err(Into::into),
            },
            &|length, scope, _, _, _| match length {
                ArrayLength::Generic(reference) => keep_const(reference, scope).map_err(Into::into),
                ArrayLength::Const(_) => Ok(length.clone()),
                ArrayLength::Error(_) | ArrayLength::Inferred => {
                    Err(TypeInstantiationError::UnresolvedType.into())
                }
            },
            &|row, source, _, depth, control| {
                map_effects_with_control(row, depth, control, &|reference, _| {
                    Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        keep_effect(reference, source)?,
                    ))
                })
            },
        )
    }
}
