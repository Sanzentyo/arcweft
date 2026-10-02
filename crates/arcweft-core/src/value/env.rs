#[cfg(test)]
use super::RuntimeRecordFieldId;
use super::{
    RuntimeEnv, RuntimeEvalError, RuntimeLocalBinding, RuntimeLocalRead, RuntimeLocalReadMode,
    RuntimeLocalSlot, RuntimeMutablePlace, RuntimeScope, RuntimeValue, runtime_value_label,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::scope::RuntimeScopeIdentity;
use crate::task::RuntimeProgramOwner;

/// Non-runnable rollback image of one native lexical environment. Values are
/// encoded under the exact immutable program lease; spare allocation scopes
/// are omitted because they contain no bindings and carry no semantics.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeEnvRollbackImage {
    scopes: Vec<RuntimeScopeRollbackImage>,
    assignment_discards: crate::line_task::RuntimeHandleDropAuthorization,
}

#[derive(Debug, PartialEq)]
pub(crate) struct RuntimePlaceWriteError {
    cause: RuntimeEvalError,
    value: Box<RuntimeValue>,
}

impl RuntimePlaceWriteError {
    pub(crate) fn into_parts(self) -> (RuntimeEvalError, RuntimeValue) {
        (self.cause, *self.value)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RuntimeScopeRollbackImage {
    identity: RuntimeScopeIdentity,
    slots: Vec<(
        RuntimeLocalDeclarationId,
        Option<super::AwbcRuntimeValueSnapshot>,
    )>,
}

impl Default for RuntimeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![RuntimeScope::default()],
            spare_scopes: Vec::new(),
            assignment_discards: crate::line_task::RuntimeHandleDropAuthorization::default(),
        }
    }
}

impl Clone for RuntimeEnv {
    fn clone(&self) -> Self {
        Self {
            scopes: self.scopes.clone(),
            spare_scopes: Vec::new(),
            assignment_discards: self.assignment_discards.clone(),
        }
    }
}

impl PartialEq for RuntimeEnv {
    fn eq(&self, other: &Self) -> bool {
        self.scopes == other.scopes && self.assignment_discards == other.assignment_discards
    }
}

impl RuntimeEnv {
    pub(crate) fn inert_rollback_image(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeEnvRollbackImage, String> {
        Ok(RuntimeEnvRollbackImage {
            assignment_discards: self.assignment_discards.clone(),
            scopes: self
                .scopes
                .iter()
                .map(|scope| {
                    Ok(RuntimeScopeRollbackImage {
                        identity: scope.identity.clone(),
                        slots: scope
                            .slots
                            .iter()
                            .map(|binding| {
                                Ok((
                                    binding.local,
                                    binding.value.as_ref().map(|value|
                                        super::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
                                            .map_err(|error| error.to_string())
                                    ).transpose()?,
                                ))
                            })
                            .collect::<Result<_, String>>()?,
                    })
                })
                .collect::<Result<_, String>>()?,
        })
    }

    pub(crate) fn from_rollback_image(
        image: RuntimeEnvRollbackImage,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            scopes: image
                .scopes
                .into_iter()
                .map(|scope| {
                    Ok(RuntimeScope {
                        identity: scope.identity,
                        slots: scope
                            .slots
                            .into_iter()
                            .map(|(local, value)| {
                                Ok(RuntimeLocalSlot {
                                    local,
                                    value: value
                                        .map(|value| {
                                            value
                                                .into_runtime_value_for_program(owner)
                                                .map_err(|error| error.to_string())
                                        })
                                        .transpose()?,
                                })
                            })
                            .collect::<Result<_, String>>()?,
                    })
                })
                .collect::<Result<_, String>>()?,
            spare_scopes: Vec::new(),
            assignment_discards: image.assignment_discards,
        })
    }
    pub(crate) fn try_duplicate_unrestricted(&self) -> Result<Self, RuntimeEvalError> {
        for (local, value) in self.bindings() {
            if !value.ownership().permits_copy() {
                return Err(RuntimeEvalError::AffineLocalCopy(local));
            }
        }
        Ok(self.clone())
    }

    pub fn push_scope(&mut self) {
        self.push_scope_with_capacity(0);
    }

    pub(crate) fn push_scope_with_capacity(&mut self, binding_capacity: usize) {
        self.push_scope_with_identity_and_capacity(
            RuntimeScopeIdentity::Anonymous,
            binding_capacity,
        );
    }

    pub(crate) fn push_scope_with_identity(&mut self, identity: RuntimeScopeIdentity) {
        self.push_scope_with_identity_and_capacity(identity, 0);
    }

    fn push_scope_with_identity_and_capacity(
        &mut self,
        identity: RuntimeScopeIdentity,
        binding_capacity: usize,
    ) {
        let mut scope = self
            .spare_scopes
            .pop()
            .unwrap_or_else(|| RuntimeScope::with_capacity(binding_capacity));
        scope.clear();
        scope.identity = identity;
        scope.reserve_bindings(binding_capacity);
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) {
        let _ = self.pop_scope_bindings();
    }

    /// Removes one lexical scope while returning its bindings to the owner
    /// that must reconcile affine resources before discarding them.
    pub(crate) fn pop_scope_bindings(&mut self) -> Vec<RuntimeLocalBinding> {
        if self.scopes.len() > 1 {
            if let Some(mut scope) = self.scopes.pop() {
                let bindings = scope.take_bindings();
                scope.clear();
                self.spare_scopes.push(scope);
                return bindings;
            }
        } else if let Some(scope) = self.scopes.last_mut() {
            let bindings = scope.take_bindings();
            scope.clear();
            return bindings;
        }
        Vec::new()
    }

    pub(crate) fn current_scope_bindings(
        &self,
    ) -> impl Iterator<Item = (RuntimeLocalDeclarationId, &RuntimeValue)> {
        self.scopes
            .last()
            .into_iter()
            .flat_map(RuntimeScope::bindings)
    }

    pub fn set(&mut self, local: RuntimeLocalDeclarationId, value: RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set(local, value);
        }
    }

    pub(crate) fn set_ref(&mut self, local: RuntimeLocalDeclarationId, value: &RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_ref(local, value);
        }
    }

    pub fn set_root(&mut self, local: RuntimeLocalDeclarationId, value: RuntimeValue) {
        self.ensure_root_scope();
        if let Some(scope) = self.scopes.first_mut() {
            scope.set(local, value);
        }
    }

    pub fn get(&self, local: RuntimeLocalDeclarationId) -> Option<&RuntimeValue> {
        self.slot(local)?.value.as_ref()
    }

    /// Removes and returns the nearest binding for one admitted local. This is
    /// the structured runtime's move boundary; affine values are never cloned
    /// before their consuming instruction commits.
    pub(crate) fn take(&mut self, local: RuntimeLocalDeclarationId) -> Option<RuntimeValue> {
        self.slot_mut(local)?.value.take()
    }

    fn slot(&self, local: RuntimeLocalDeclarationId) -> Option<&RuntimeLocalSlot> {
        self.scopes.iter().rev().find_map(|scope| scope.slot(local))
    }

    fn slot_mut(&mut self, local: RuntimeLocalDeclarationId) -> Option<&mut RuntimeLocalSlot> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.slot_mut(local))
    }

    /// Executes the already-selected local-use transfer and checks live value
    /// ownership before any copy. A moved binding is absent for later reads.
    pub(crate) fn read(
        &mut self,
        read: RuntimeLocalRead,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let slot = self
            .slot_mut(read.local())
            .ok_or(RuntimeEvalError::UnknownLocal(read.local()))?;
        match read.mode() {
            RuntimeLocalReadMode::Copy => {
                let value = slot
                    .value
                    .as_ref()
                    .ok_or(RuntimeEvalError::UninitializedLocal(read.local()))?;
                if !value.ownership().permits_copy() {
                    return Err(RuntimeEvalError::AffineLocalCopy(read.local()));
                }
                Ok(value.clone())
            }
            RuntimeLocalReadMode::Move => slot
                .value
                .take()
                .ok_or(RuntimeEvalError::UninitializedLocal(read.local())),
        }
    }

    /// Pops from the nearest binding without moving or cloning the sequence
    /// value itself. Checked expression admission restricts this operation to
    /// Vec locals and parameters.
    pub(crate) fn pop_sequence_front(
        &mut self,
        place: RuntimeMutablePlace,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        Ok(self.sequence_mut(place)?.pop_front())
    }

    pub(crate) fn push_vector_item(
        &mut self,
        place: RuntimeMutablePlace,
        value: RuntimeValue,
    ) -> Result<(), RuntimeEvalError> {
        self.sequence_mut(place)?.push_vector_item(value);
        Ok(())
    }

    pub(crate) fn pop_vector_item(
        &mut self,
        place: RuntimeMutablePlace,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        Ok(self.sequence_mut(place)?.pop_vector_item())
    }

    fn sequence_mut(
        &mut self,
        place: RuntimeMutablePlace,
    ) -> Result<&mut super::RuntimeSeq, RuntimeEvalError> {
        let local = match place {
            RuntimeMutablePlace::Local(local) => local,
            RuntimeMutablePlace::NominalField { base, .. } => base,
        };
        let binding = self
            .slot_mut(local)
            .ok_or(RuntimeEvalError::UnknownLocal(local))?;
        let value = binding
            .value
            .as_mut()
            .ok_or(RuntimeEvalError::UninitializedLocal(local))?;
        match place {
            RuntimeMutablePlace::Local(_) => match value {
                RuntimeValue::Seq(sequence) => Ok(sequence),
                value => Err(RuntimeEvalError::ExpectedSequence(runtime_value_label(
                    value,
                ))),
            },
            RuntimeMutablePlace::NominalField { field, .. } => match value {
                RuntimeValue::NominalRecord(record) => record.sequence_field_mut(field),
                value => Err(RuntimeEvalError::ExpectedSequence(runtime_value_label(
                    value,
                ))),
            },
        }
    }

    pub(crate) fn bindings(
        &self,
    ) -> impl Iterator<Item = (RuntimeLocalDeclarationId, &RuntimeValue)> {
        self.scopes.iter().flat_map(RuntimeScope::bindings)
    }

    pub(crate) fn assign_place(
        &mut self,
        place: RuntimeMutablePlace,
        value: RuntimeValue,
    ) -> Result<Option<RuntimeValue>, RuntimePlaceWriteError> {
        let inspected = (|| {
            let slot = self
                .slot(place.local())
                .ok_or(RuntimeEvalError::UnknownLocal(place.local()))?;
            match place {
                RuntimeMutablePlace::Local(_) => Ok(slot.value.as_ref()),
                RuntimeMutablePlace::NominalField { field, .. } => {
                    let base = slot
                        .value
                        .as_ref()
                        .ok_or(RuntimeEvalError::UninitializedLocal(place.local()))?;
                    base.record_field(field).map(Some).ok_or_else(|| {
                        RuntimeEvalError::InvalidFieldAssignment {
                            field: field.zero_based().to_string(),
                            value: runtime_value_label(base),
                        }
                    })
                }
            }
        })();
        let handles = inspected.and_then(|displaced| match displaced {
            Some(value) => value
                .affine_line_handles()
                .map_err(|_| RuntimeEvalError::InvalidDiscardGraph),
            None => Ok(Vec::new()),
        });
        let handles = match handles {
            Ok(handles) => handles,
            Err(cause) => {
                return Err(RuntimePlaceWriteError {
                    cause,
                    value: Box::new(value),
                });
            }
        };
        if self.assignment_discards.authorize_handles(handles).is_err() {
            return Err(RuntimePlaceWriteError {
                cause: RuntimeEvalError::InvalidDiscardGraph,
                value: Box::new(value),
            });
        }
        let displaced = match place {
            RuntimeMutablePlace::Local(local) => self
                .slot_mut(local)
                .expect("inspected local remains in the exclusive environment")
                .value
                .replace(value),
            RuntimeMutablePlace::NominalField { base, field } => Some(
                self.slot_mut(base)
                    .and_then(|slot| slot.value.as_mut())
                    .expect("inspected field owner remains initialized")
                    .replace_record_field(field, value)
                    .expect("inspected field retains its defining-order coordinate"),
            ),
        };
        Ok(displaced)
    }

    pub(crate) fn take_assignment_discard_authorization(
        &mut self,
    ) -> crate::line_task::RuntimeHandleDropAuthorization {
        std::mem::take(&mut self.assignment_discards)
    }

    pub fn bindings_snapshot(&self) -> Vec<RuntimeLocalBinding> {
        self.bindings()
            .map(|(local, value)| RuntimeLocalBinding {
                local,
                value: value.clone(),
            })
            .collect()
    }

    pub(crate) fn into_bindings(self) -> Vec<RuntimeLocalBinding> {
        self.scopes
            .into_iter()
            .flat_map(|mut scope| scope.take_bindings())
            .collect()
    }

    pub(crate) fn replace_scopes_with_bindings(
        &mut self,
        scopes: impl IntoIterator<Item = Vec<RuntimeLocalBinding>>,
    ) {
        self.spare_scopes
            .extend(self.scopes.drain(..).map(|mut scope| {
                scope.clear();
                scope
            }));

        for bindings in scopes {
            let mut scope = self
                .spare_scopes
                .pop()
                .unwrap_or_else(|| RuntimeScope::with_capacity(bindings.len()));
            scope.clear();
            scope.reserve_bindings(bindings.len());
            for binding in bindings {
                scope.set(binding.local, binding.value);
            }
            self.scopes.push(scope);
        }

        if self.scopes.is_empty() {
            self.push_scope();
        }
    }

    pub fn bind_all(&mut self, bindings: impl IntoIterator<Item = RuntimeLocalBinding>) {
        for binding in bindings {
            self.set(binding.local, binding.value);
        }
    }

    pub(crate) fn bind_all_ref(&mut self, bindings: &[RuntimeLocalBinding]) {
        for binding in bindings {
            self.set_ref(binding.local, &binding.value);
        }
    }

    pub fn bind_all_root(&mut self, bindings: impl IntoIterator<Item = RuntimeLocalBinding>) {
        for binding in bindings {
            self.set_root(binding.local, binding.value);
        }
    }

    pub fn bind_all_root_ref(&mut self, bindings: &[RuntimeLocalBinding]) {
        if self.replace_root_bindings_ref(bindings) {
            return;
        }
        for binding in bindings {
            self.set_root_ref(binding.local, &binding.value);
        }
    }

    fn replace_root_bindings_ref(&mut self, bindings: &[RuntimeLocalBinding]) -> bool {
        if self.scopes.is_empty() {
            return bindings.is_empty();
        }
        self.scopes
            .first_mut()
            .is_some_and(|scope| scope.replace_binding_values_ref(bindings))
    }

    fn set_root_ref(&mut self, local: RuntimeLocalDeclarationId, value: &RuntimeValue) {
        self.ensure_root_scope();
        if let Some(scope) = self.scopes.first_mut() {
            scope.set_ref(local, value);
        }
    }

    fn ensure_root_scope(&mut self) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
    }
}

impl RuntimeScope {
    fn with_capacity(binding_capacity: usize) -> Self {
        Self {
            identity: RuntimeScopeIdentity::Anonymous,
            slots: Vec::with_capacity(binding_capacity),
        }
    }

    fn reserve_bindings(&mut self, binding_capacity: usize) {
        let additional = binding_capacity.saturating_sub(self.slots.capacity());
        self.slots.reserve(additional);
    }

    fn set(&mut self, local: RuntimeLocalDeclarationId, value: RuntimeValue) {
        if let Some(binding) = self.slot_mut(local) {
            binding.value = Some(value);
        } else {
            self.slots.push(RuntimeLocalSlot {
                local,
                value: Some(value),
            });
        }
    }

    fn set_ref(&mut self, local: RuntimeLocalDeclarationId, value: &RuntimeValue) {
        self.set(local, value.clone());
    }

    fn slot(&self, local: RuntimeLocalDeclarationId) -> Option<&RuntimeLocalSlot> {
        self.slots
            .iter()
            .rev()
            .find(|binding| binding.local == local)
    }

    fn slot_mut(&mut self, local: RuntimeLocalDeclarationId) -> Option<&mut RuntimeLocalSlot> {
        self.slots
            .iter_mut()
            .rev()
            .find(|binding| binding.local == local)
    }

    fn bindings(&self) -> impl Iterator<Item = (RuntimeLocalDeclarationId, &RuntimeValue)> {
        self.slots
            .iter()
            .filter_map(|slot| slot.value.as_ref().map(|value| (slot.local, value)))
    }

    fn take_bindings(&mut self) -> Vec<RuntimeLocalBinding> {
        std::mem::take(&mut self.slots)
            .into_iter()
            .filter_map(|slot| {
                slot.value.map(|value| RuntimeLocalBinding {
                    local: slot.local,
                    value,
                })
            })
            .collect()
    }

    fn clear(&mut self) {
        self.identity = RuntimeScopeIdentity::Anonymous;
        self.slots.clear();
    }

    fn replace_binding_values_ref(&mut self, bindings: &[RuntimeLocalBinding]) -> bool {
        if self.slots.len() != bindings.len()
            || !self
                .slots
                .iter()
                .zip(bindings)
                .all(|(current, next)| current.local == next.local)
        {
            return false;
        }
        for (current, next) in self.slots.iter_mut().zip(bindings) {
            current.value = Some(next.value.clone());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
    use crate::value::{RuntimeNominalRecordValue, RuntimeSeq};
    use std::num::NonZeroU32;

    fn local(ordinal: u32) -> RuntimeLocalDeclarationId {
        RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(ordinal).unwrap())
    }

    #[test]
    fn checked_local_reads_copy_only_unrestricted_values_and_transfer_moves_once() {
        let affine = local(1);
        let scalar = local(2);
        let mut env = RuntimeEnv::default();
        let need = RuntimeValue::Need(crate::task::NeedId("need.local-read".to_owned()));
        env.set(affine, need.clone());
        env.set(scalar, RuntimeValue::Bool(true));

        let copy_affine = RuntimeLocalRead::from_admitted_parts(affine, RuntimeLocalReadMode::Copy);
        assert_eq!(
            env.read(copy_affine),
            Err(RuntimeEvalError::AffineLocalCopy(affine))
        );
        assert_eq!(env.get(affine), Some(&need));

        let copy_scalar = RuntimeLocalRead::from_admitted_parts(scalar, RuntimeLocalReadMode::Copy);
        assert_eq!(env.read(copy_scalar), Ok(RuntimeValue::Bool(true)));
        assert_eq!(env.read(copy_scalar), Ok(RuntimeValue::Bool(true)));

        let move_affine = RuntimeLocalRead::from_admitted_parts(affine, RuntimeLocalReadMode::Move);
        assert_eq!(env.read(move_affine), Ok(need));
        assert_eq!(
            env.read(move_affine),
            Err(RuntimeEvalError::UninitializedLocal(affine))
        );
    }

    #[test]
    fn moved_declaration_reinitializes_in_its_original_scope_after_rollback() {
        let outer = local(1);
        let inner = local(2);
        let mut env = RuntimeEnv::default();
        env.set(outer, RuntimeValue::Bool(false));
        assert_eq!(env.take(outer), Some(RuntimeValue::Bool(false)));
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
            crate::awbc::schema::AwbcProgram::default(),
        ));
        let image = env.inert_rollback_image(&owner).unwrap();
        assert_eq!(image.scopes[0].slots, [(outer, None)]);
        let mut restored = RuntimeEnv::from_rollback_image(image, &owner).unwrap();
        restored.push_scope();
        restored.set(inner, RuntimeValue::Bool(false));
        let displaced = restored
            .assign_place(RuntimeMutablePlace::Local(outer), RuntimeValue::Bool(true))
            .unwrap();
        assert_eq!(displaced, None);
        restored.pop_scope();
        assert_eq!(restored.get(outer), Some(&RuntimeValue::Bool(true)));
        assert!(restored.get(inner).is_none());
    }

    #[test]
    fn empty_nearest_slot_does_not_resolve_an_outer_same_identity_value() {
        let source = local(1);
        let mut env = RuntimeEnv::default();
        env.set(source, RuntimeValue::Bool(false));
        env.push_scope();
        env.set(source, RuntimeValue::Bool(true));
        assert_eq!(env.take(source), Some(RuntimeValue::Bool(true)));
        assert!(env.get(source).is_none());
        assert!(env.take(source).is_none());
        let read = RuntimeLocalRead::from_admitted_parts(source, RuntimeLocalReadMode::Copy);
        assert_eq!(
            env.read(read),
            Err(RuntimeEvalError::UninitializedLocal(source))
        );
        let displaced = env
            .assign_place(RuntimeMutablePlace::Local(source), RuntimeValue::Bool(true))
            .unwrap();
        assert_eq!(displaced, None);
        assert_eq!(env.get(source), Some(&RuntimeValue::Bool(true)));
        env.pop_scope();
        assert_eq!(env.get(source), Some(&RuntimeValue::Bool(false)));
    }

    #[test]
    fn rejected_place_write_retains_the_owned_input_and_existing_slots() {
        let mut env = RuntimeEnv::default();
        let target = local(1);
        env.set(target, RuntimeValue::Bool(false));
        let input = RuntimeValue::Need(crate::task::NeedId("need.rejected-write".to_owned()));
        let error = env
            .assign_place(RuntimeMutablePlace::Local(local(2)), input)
            .unwrap_err();
        let (cause, retained) = error.into_parts();
        assert_eq!(cause, RuntimeEvalError::UnknownLocal(local(2)));
        assert_eq!(
            retained,
            RuntimeValue::Need(crate::task::NeedId("need.rejected-write".to_owned()))
        );
        assert_eq!(env.get(target), Some(&RuntimeValue::Bool(false)));
        assert!(env.get(local(2)).is_none());
    }

    #[test]
    fn scopes_resolve_plan_local_ids_without_names() {
        let root = local(1);
        let shadow = local(2);
        let mut env = RuntimeEnv::default();
        env.set_root(root, RuntimeValue::Bool(true));
        env.push_scope();
        env.set(shadow, RuntimeValue::String("inner".to_owned()));

        assert_eq!(env.get(root), Some(&RuntimeValue::Bool(true)));
        assert_eq!(
            env.get(shadow),
            Some(&RuntimeValue::String("inner".to_owned()))
        );
        assert_eq!(
            env.bindings_snapshot(),
            vec![
                RuntimeLocalBinding {
                    local: root,
                    value: RuntimeValue::Bool(true),
                },
                RuntimeLocalBinding {
                    local: shadow,
                    value: RuntimeValue::String("inner".to_owned()),
                },
            ]
        );
    }

    #[test]
    fn reused_runtime_scope_owns_and_clears_its_typed_identity() {
        let identity = RuntimeScopeIdentity::Named(
            arcweft_id::DeclarationName::try_new("window").expect("valid scope name"),
        );
        let mut env = RuntimeEnv::default();
        env.push_scope_with_identity(identity.clone());
        assert_eq!(
            env.scopes.last().map(|scope| &scope.identity),
            Some(&identity)
        );

        env.pop_scope();
        env.push_scope();
        assert_eq!(
            env.scopes.last().map(|scope| &scope.identity),
            Some(&RuntimeScopeIdentity::Anonymous)
        );
    }

    #[test]
    fn field_assignment_uses_nominal_defining_order_identity() {
        let local = local(1);
        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap();
        let mut env = RuntimeEnv::default();
        env.set(
            local,
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                RuntimeNominalTypeId::try_new("game.Pair").unwrap(),
                crate::pattern::RuntimeSemanticTypeId::from_bytes([9; 32]),
                TypeLayoutHash::from_bytes([9; 32]),
                vec![
                    RuntimeValue::Bool(true),
                    RuntimeValue::String("old".to_owned()),
                ],
            )),
        );

        assert_eq!(
            env.assign_place(
                RuntimeMutablePlace::NominalField { base: local, field },
                RuntimeValue::String("new".to_owned())
            ),
            Ok(Some(RuntimeValue::String("old".to_owned())))
        );
        let Some(RuntimeValue::NominalRecord(record)) = env.get(local) else {
            panic!("nominal record remains bound");
        };
        assert_eq!(
            record.field(field),
            Some(&RuntimeValue::String("new".to_owned()))
        );
    }

    #[test]
    fn sequence_pop_front_mutates_the_nearest_binding_and_moves_values() {
        let local = local(9);
        let mut env = RuntimeEnv::default();
        env.set_root(
            local,
            RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::String(
                "outer".to_owned(),
            )])),
        );
        env.push_scope();
        env.set(
            local,
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::String("first".to_owned()),
                RuntimeValue::String("second".to_owned()),
            ])),
        );

        assert_eq!(
            env.pop_sequence_front(RuntimeMutablePlace::Local(local)),
            Ok(Some(RuntimeValue::String("first".to_owned())))
        );
        assert_eq!(
            env.get(local),
            Some(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::String("second".to_owned(),)
            ])))
        );

        env.pop_scope();
        assert_eq!(
            env.get(local),
            Some(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::String("outer".to_owned(),)
            ])))
        );
        assert_eq!(
            env.pop_sequence_front(RuntimeMutablePlace::Local(local)),
            Ok(Some(RuntimeValue::String("outer".to_owned())))
        );
        assert_eq!(
            env.pop_sequence_front(RuntimeMutablePlace::Local(local)),
            Ok(None)
        );
    }

    #[test]
    fn sequence_pop_front_mutates_a_nominal_vec_field_in_place() {
        let local = local(10);
        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
        let mut env = RuntimeEnv::default();
        env.set(
            local,
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                RuntimeNominalTypeId::try_new("game.Container").unwrap(),
                crate::pattern::RuntimeSemanticTypeId::from_bytes([10; 32]),
                TypeLayoutHash::from_bytes([10; 32]),
                vec![RuntimeValue::Seq(RuntimeSeq::values(vec![
                    RuntimeValue::String("first".to_owned()),
                    RuntimeValue::String("second".to_owned()),
                ]))],
            )),
        );

        let place = RuntimeMutablePlace::NominalField { base: local, field };
        assert_eq!(
            env.pop_sequence_front(place),
            Ok(Some(RuntimeValue::String("first".to_owned())))
        );
        let Some(RuntimeValue::NominalRecord(record)) = env.get(local) else {
            panic!("nominal record remains bound after field mutation");
        };
        let Some(RuntimeValue::Seq(sequence)) = record.field(field) else {
            panic!("Vec stays in its nominal field");
        };
        assert_eq!(sequence.len(), 1);
        assert_eq!(
            sequence.value_at(0),
            RuntimeValue::String("second".to_owned())
        );
    }

    #[test]
    fn vector_end_mutation_targets_the_nearest_local_or_nominal_field() {
        let local = local(11);
        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
        let mut env = RuntimeEnv::default();
        env.set_root(local, RuntimeValue::Seq(RuntimeSeq::dense_i32(vec![1])));
        env.push_scope();
        env.set(
            local,
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                RuntimeNominalTypeId::try_new("game.Items").unwrap(),
                crate::pattern::RuntimeSemanticTypeId::from_bytes([11; 32]),
                TypeLayoutHash::from_bytes([11; 32]),
                vec![RuntimeValue::Seq(RuntimeSeq::dense_i32(vec![2]))],
            )),
        );
        let place = RuntimeMutablePlace::NominalField { base: local, field };
        assert_eq!(env.push_vector_item(place, RuntimeValue::i32(3)), Ok(()));
        assert_eq!(env.pop_vector_item(place), Ok(Some(RuntimeValue::i32(3))));
        env.pop_scope();
        let place = RuntimeMutablePlace::Local(local);
        assert_eq!(env.pop_vector_item(place), Ok(Some(RuntimeValue::i32(1))));
        assert_eq!(env.pop_vector_item(place), Ok(None));
    }
}
