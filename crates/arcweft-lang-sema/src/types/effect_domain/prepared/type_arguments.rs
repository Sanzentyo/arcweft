//! Simultaneous application of retained arguments to a declaration template.

use super::*;
use crate::effect_row::prepared::{PreparedEffectGraph, PreparedEffectTermEdit};
use crate::types::structural_projection::ScopedTypeProjection;
use crate::types::{
    ArrayLength, CanonicalEffects, GenericDeclarationBinder, GenericTypeReference,
    TypeInstantiationError,
};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// One borrowed formal index for all templates of the same application. Its
/// inventory shape is checked once and contains no type copies. The owning
/// phase still admits the arguments in Γ; copied live terms retain their issuer.
pub(crate) struct PreparedTypeArguments<'a> {
    declaration: &'a GenericDeclarationBinder,
    argument_scope: &'a GenericScope,
    bindings: BTreeMap<&'a GenericTypeReference, &'a TypeKind<PreparedEffects>>,
}

impl<'a> PreparedTypeArguments<'a> {
    pub(crate) fn new<C: TypeProjectionControl>(
        declaration: &'a GenericDeclarationBinder,
        arguments: &'a [TypeKind<PreparedEffects>],
        argument_scope: &'a GenericScope,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        control.check().map_err(|error| {
            TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
        })?;
        let formals = declaration.type_parameters();
        if formals.len() != arguments.len() {
            return Err(TypeInstantiationError::TypeArgumentCount {
                expected: formals.len(),
                actual: arguments.len(),
            }
            .into());
        }
        argument_scope.extension_depth(declaration.template_scope())?;
        let mut bindings = BTreeMap::new();
        for (parameter, argument) in formals.iter().zip(arguments) {
            control.visit_binding().map_err(|error| {
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
            })?;
            parameter.template_key(declaration.template_scope(), declaration.template_scope())?;
            if bindings.insert(parameter, argument).is_some() {
                return Err(TypeInstantiationError::DuplicateTypeArgumentParameter {
                    parameter: parameter.clone(),
                }
                .into());
            }
        }
        Ok(Self {
            declaration,
            argument_scope,
            bindings,
        })
    }

    /// A containing operation owns the edit until all of its fields succeed.
    pub(crate) fn project_in<C: TypeProjectionControl>(
        &self,
        template: &TypeKind,
        edit: &mut PreparedEffectTermEdit<'_>,
        control: &mut C,
    ) -> Result<
        TypeKind<PreparedEffects>,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        let result = template.project_structure(
            self.declaration.template_scope(),
            self.argument_scope,
            1,
            &mut Arguments {
                template_scope: self.declaration.template_scope(),
                argument_scope: self.argument_scope,
                bindings: &self.bindings,
                edit,
                control,
            },
        );
        if result.is_err() {
            edit.invalidate();
        }
        result
    }
}

struct Arguments<'a, 'g, C> {
    template_scope: &'a GenericScope,
    argument_scope: &'a GenericScope,
    bindings: &'a BTreeMap<&'a GenericTypeReference, &'a TypeKind<PreparedEffects>>,
    edit: &'a mut PreparedEffectTermEdit<'g>,
    control: &'a mut C,
}

impl<C: TypeProjectionControl> TypeProjectionControl for Arguments<'_, '_, C> {
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;
    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C: TypeProjectionControl> ScopedTypeProjection<CanonicalEffects, PreparedEffects>
    for Arguments<'_, '_, C>
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Self::Error>> {
        if let Some(key) = reference.template_key(self.template_scope, source)? {
            self.visit_binding().map_err(TypeProjectionError::Control)?;
            if let Some(argument) = self.bindings.get(&key) {
                // The argument owns its incoming scope. The lift never looks
                // up declaration keys in this value a second time.
                return argument.lift_under_in(
                    self.argument_scope,
                    target,
                    depth,
                    self.edit,
                    self.control,
                );
            }
        }
        Ok(TypeKind::GenericParam(reference.lift_into(
            self.template_scope,
            source,
            target,
        )?))
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        self.visit_node(TypeProjectionNodeKind::Const, depth)
            .map_err(TypeProjectionError::Control)?;
        match length {
            ArrayLength::Generic(reference) => Ok(ArrayLength::Generic(reference.lift_into(
                self.template_scope,
                source,
                target,
            )?)),
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        }
    }

    fn project_row(
        &mut self,
        row: &EffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        visit_effect_row(self.control, row, depth)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
        let row = PreparedEffectRow::Canonical(row.clone());
        self.edit
            .map_row_parameters(
                &row,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth,
                },
                &mut |reference, _| {
                    let reference = reference
                        .lift_into(self.template_scope, source, target)
                        .map_err(|error| {
                            PreparedEffectFailure::Control(TypeProjectionError::from(error))
                        })?;
                    Ok(Some(PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        reference,
                    ))))
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        visit_effect_predicate(self.control, predicate, depth)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
        let predicate = PreparedEffectPredicate::Canonical(predicate.clone());
        self.edit
            .map_predicate_parameters(
                &predicate,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth,
                },
                &mut |reference, _| {
                    let reference = reference
                        .lift_into(self.template_scope, source, target)
                        .map_err(|error| {
                            PreparedEffectFailure::Control(TypeProjectionError::from(error))
                        })?;
                    Ok(Some(PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        reference,
                    ))))
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }
}

impl TypeKind {
    /// Apply the exact declaration's type formals while retaining live caller
    /// arguments. Other references keep their source identity. No inference,
    /// Γ admission, body preparation, or completed specialization occurs here.
    pub(crate) fn prepare_type_arguments_with_control<C: TypeProjectionControl>(
        &self,
        declaration: &GenericDeclarationBinder,
        arguments: &[TypeKind<PreparedEffects>],
        argument_scope: &GenericScope,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<
        TypeKind<PreparedEffects>,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        let arguments =
            PreparedTypeArguments::new(declaration, arguments, argument_scope, control)?;
        let mut edit = graph.edit_terms();
        let result = arguments.project_in(self, &mut edit, control)?;
        edit.commit()
            .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error.into()))?;
        Ok(result)
    }
}
