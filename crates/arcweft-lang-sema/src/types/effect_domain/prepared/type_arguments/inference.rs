//! Infer declaration-owned arguments without closing the original effect graph.

use std::sync::Arc;

use arcweft_lang_hir::symbol::nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId};

use super::*;
use crate::types::{
    PreparedTypeRetentionFailure, ProjectNominalType, constraints::lexical::ConstraintLexicalScope,
    structural_projection::child_depth,
};

pub(crate) struct PreparedNominalArgumentInference {
    declaration: ProjectNominalDeclarationId,
    formals: GenericDeclarationBinder,
    caller: Arc<ConstraintLexicalScope>,
    positions: BTreeMap<GenericTypeReference, usize>,
    arguments: Vec<Option<TypeKind<PreparedEffects>>>,
}

impl PreparedNominalArgumentInference {
    pub(crate) fn new<C: TypeProjectionControl>(
        declaration: &ProjectNominalDeclaration,
        caller: &GenericScope,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let formals = GenericDeclarationBinder::for_project_nominal(declaration, control)?;
        let caller = Arc::new(ConstraintLexicalScope::from_source_shape(caller, control)?);
        let mut positions = BTreeMap::new();
        let mut arguments = Vec::new();
        for (position, formal) in formals.type_parameters().iter().enumerate() {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            positions.insert(formal.clone(), position);
            arguments.push(None);
        }
        Ok(Self {
            declaration: declaration.id().clone(),
            formals,
            caller,
            positions,
            arguments,
        })
    }

    pub(crate) fn seed(
        &mut self,
        position: usize,
        ty: TypeKind<PreparedEffects>,
    ) -> Result<(), TypeInstantiationError> {
        let expected = self.arguments.len();
        let argument =
            self.arguments
                .get_mut(position)
                .ok_or(TypeInstantiationError::TypeArgumentCount {
                    expected,
                    actual: position.saturating_add(1),
                })?;
        if argument.is_some() {
            return Err(TypeInstantiationError::SpecializationConflict);
        }
        *argument = Some(ty);
        Ok(())
    }

    /// A field is contextual only when every declaration argument it uses
    /// is known. The same scoped projector also applies complete applications.
    pub(crate) fn project_field<C: TypeProjectionControl>(
        &self,
        template: &TypeKind,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<
        Option<TypeKind<PreparedEffects>>,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        let index = PreparedTypeArguments::index(
            &self.formals,
            self.arguments.iter().map(Option::as_ref),
            self.caller.shape(),
            control,
        )?;
        let mut edit = graph.edit_terms();
        let projected = match index.project_in(template, &mut edit, control) {
            Ok(projected) => projected,
            Err(TypeProjectionError::Instantiation(TypeInstantiationError::UnboundType {
                ..
            })) => return Ok(None),
            Err(error) => return Err(error),
        };
        edit.commit()
            .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error.into()))?;
        Ok(Some(projected))
    }

    /// Observe this field in source order. Earlier arguments are immutable;
    /// the projected field must subsequently match the actual source type.
    /// A captured type cannot carry binders belonging to a nested value.
    pub(crate) fn observe_field<C: TypeProjectionControl>(
        &mut self,
        template: &TypeKind,
        actual: &TypeKind<PreparedEffects>,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<bool, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        let visit = |control: &mut C, depth| {
            control
                .visit_node(TypeProjectionNodeKind::Type, depth)
                .map_err(|error| {
                    TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(
                        error,
                    ))
                })
        };
        visit(control, 1)?;
        let mut pending = vec![(template, actual, Arc::clone(&self.caller), 1)];
        let mut captured = BTreeMap::new();
        let mut edit = graph.edit_terms();
        while let Some((template, actual, scope, depth)) = pending.pop() {
            if matches!(actual, TypeKind::Error(_))
                || matches!(actual, TypeKind::Named(name) if name == "_")
            {
                continue;
            }
            if let TypeKind::GenericParam(formal) = template {
                let Some(&position) = self.positions.get(formal) else {
                    continue;
                };
                if self.arguments[position].is_none() && !captured.contains_key(&position) {
                    let argument = if Arc::ptr_eq(&self.caller, &scope) {
                        actual
                            .retain_effects(self.caller.shape(), edit.read(), control)
                            .map_err(|error| match error {
                                TypeProjectionError::Instantiation(error) => {
                                    TypeProjectionError::Instantiation(error)
                                }
                                TypeProjectionError::Control(
                                    PreparedTypeRetentionFailure::Control(error),
                                ) => TypeProjectionError::Control(
                                    PreparedTypeEffectProjectionFailure::Control(error),
                                ),
                                TypeProjectionError::Control(
                                    PreparedTypeRetentionFailure::Effect(error),
                                ) => PreparedTypeEffectProjectionFailure::from_effect(error.into()),
                            })?
                    } else {
                        match self
                            .caller
                            .transport_type_in(&scope, actual, &mut edit, control)
                        {
                            Ok(argument) => argument,
                            Err(TypeProjectionError::Instantiation(
                                TypeInstantiationError::SpecializationScopeMismatch,
                            )) => return Ok(false),
                            Err(error) => return Err(error),
                        }
                    };
                    captured.insert(position, argument);
                }
                continue;
            }
            let template = template.constraint_shape();
            let actual = actual.constraint_shape();
            if !template.same_header(actual) {
                continue;
            }
            let binder = actual.binder();
            let scope = if binder.is_empty() {
                scope
            } else {
                for _ in scope.shape().binders() {
                    visit(control, depth)?;
                }
                visit(control, depth)?;
                Arc::new(scope.with_binder(binder))
            };
            let child_depth = child_depth(depth)?;
            let start = pending.len();
            for (template, actual) in template.children().zip(actual.children()) {
                visit(control, child_depth)?;
                pending.push((template, actual, Arc::clone(&scope), child_depth));
            }
            pending[start..].reverse();
        }
        edit.commit()
            .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error.into()))?;
        for (position, argument) in captured {
            self.arguments[position] = Some(argument);
        }
        Ok(true)
    }

    pub(crate) fn into_nominal(self) -> Option<ProjectNominalType<PreparedEffects>> {
        let arguments = self.arguments.into_iter().collect::<Option<Vec<_>>>()?;
        Some(ProjectNominalType::new(self.declaration, arguments))
    }
}
