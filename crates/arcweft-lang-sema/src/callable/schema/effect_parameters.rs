//! Declaration of omitted input effect parameters in the signature itself.

use std::sync::Arc;

use super::{
    CallableGenericParameterAuthority, CallableGenericParameterIssuer, CallableParameterAdmission,
    CallableParameterGroup, CallableSchemaError,
};
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        GenericBinder, GenericEffectParameterId, GenericEffectReference, GenericParameterKind,
        GenericScope, GenericScopeError, TypeKind, constraints::TypeConstraintShape,
    },
};

impl CallableGenericParameterIssuer {
    /// Each omitted input row receives one declaration or lexical identity.
    /// Result/body inference is a separate obligation and never creates an
    /// implicit universal parameter here.
    pub(super) fn seal_input_effect_parameters(
        &mut self,
        groups: &mut [CallableParameterGroup],
    ) -> Result<(), CallableSchemaError> {
        let mut pending = groups
            .iter()
            .flat_map(|group| {
                group
                    .parameters()
                    .iter()
                    .filter_map(|parameter| parameter.declared_type())
            })
            .collect::<Vec<_>>();
        let mut count = 0usize;
        while let Some(ty) = pending.pop() {
            let shape = ty.constraint_shape();
            if matches!(shape, TypeConstraintShape::Function { effects, .. } if !effects.is_known())
            {
                count = count
                    .checked_add(1)
                    .ok_or(CallableSchemaError::InvalidCandidateIssuer)?;
            }
            pending.extend(shape.children());
        }
        if count == 0 {
            return Ok(());
        }
        let additional =
            u32::try_from(count).map_err(|_| GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Effect,
                count,
            })?;
        let mut slot = match &mut self.authority {
            CallableGenericParameterAuthority::Empty => {
                self.authority = CallableGenericParameterAuthority::FunctionScheme(
                    GenericBinder::new(0, 0, additional),
                );
                0
            }
            CallableGenericParameterAuthority::Declaration { effect_count, .. } => {
                let first = *effect_count;
                *effect_count = effect_count
                    .checked_add(additional)
                    .ok_or(CallableSchemaError::InvalidCandidateIssuer)?;
                first
            }
            CallableGenericParameterAuthority::FunctionScheme(binder) => {
                let first = binder.effects();
                *binder = binder.checked_append(GenericBinder::new(0, 0, additional))?;
                first
            }
        };
        let scope = self.template_scope();
        for group in groups {
            for parameter in Arc::make_mut(&mut group.parameters) {
                if let CallableParameterAdmission::Checked { declared, .. } =
                    &mut parameter.admission
                {
                    *declared = self.declare_input_effects(declared, &scope, &mut slot)?;
                }
            }
        }
        Ok(())
    }

    fn input_effect_reference(
        &self,
        slot: u32,
        scope: &GenericScope,
        incoming_depth: usize,
    ) -> Result<GenericEffectReference, CallableSchemaError> {
        if let Some(owner) = self.generic_owner() {
            return Ok(GenericEffectParameterId::new(owner, slot).into());
        }
        let depth = u32::try_from(scope.binders().len() - incoming_depth)
            .map_err(|_| CallableSchemaError::InvalidCandidateIssuer)?;
        scope.bound_effect(depth, slot).map_err(Into::into)
    }

    fn declare_input_effects(
        &self,
        ty: &TypeKind,
        incoming: &GenericScope,
        slot: &mut u32,
    ) -> Result<TypeKind, CallableSchemaError> {
        enum Task<'a> {
            Enter(&'a TypeKind, GenericScope),
            Finish(TypeConstraintShape<'a>, usize, Option<EffectRow>),
        }
        let mut pending = vec![Task::Enter(ty, incoming.clone())];
        let mut completed = Vec::new();
        while let Some(task) = pending.pop() {
            match task {
                Task::Enter(ty, scope) => {
                    let shape = ty.constraint_shape();
                    if matches!(shape, TypeConstraintShape::Unresolved) {
                        // Preserve the diagnostic-bearing type. Declaring an
                        // effect parameter does not resolve a recovered type.
                        completed.push(ty.clone());
                        continue;
                    }
                    let scope = scope.with_binder(shape.binder());
                    let replacement = if matches!(
                        shape,
                        TypeConstraintShape::Function { effects, .. } if !effects.is_known()
                    ) {
                        let reference =
                            self.input_effect_reference(*slot, &scope, incoming.binders().len())?;
                        *slot = slot
                            .checked_add(1)
                            .ok_or(CallableSchemaError::InvalidCandidateIssuer)?;
                        Some(EffectRow::open(EffectSet::new(), reference))
                    } else {
                        None
                    };
                    let children = shape.children().collect::<Vec<_>>();
                    pending.push(Task::Finish(shape, children.len(), replacement));
                    pending.extend(
                        children
                            .into_iter()
                            .rev()
                            .map(|child| Task::Enter(child, scope.clone())),
                    );
                }
                Task::Finish(shape, count, replacement) => {
                    let children = completed.split_off(completed.len() - count);
                    let mut ty = shape
                        .rebuild(children)
                        .expect("one result for each admitted, resolved structural child");
                    if let Some(row) = replacement {
                        let TypeKind::Function { effects, .. } = &mut ty else {
                            unreachable!("only function shapes carry effect rows");
                        };
                        *effects = row;
                    }
                    completed.push(ty);
                }
            }
        }
        Ok(completed.pop().expect("one input root"))
    }
}
