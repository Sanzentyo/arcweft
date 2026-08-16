use super::{RuntimeEnv, RuntimeLocalBinding, RuntimeRecordFieldId, RuntimeScope, RuntimeValue};
use crate::runtime_id::RuntimeLocalDeclarationId;

impl Default for RuntimeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![RuntimeScope::default()],
            spare_scopes: Vec::new(),
        }
    }
}

impl Clone for RuntimeEnv {
    fn clone(&self) -> Self {
        Self {
            scopes: self.scopes.clone(),
            spare_scopes: Vec::new(),
        }
    }
}

impl PartialEq for RuntimeEnv {
    fn eq(&self, other: &Self) -> bool {
        self.scopes == other.scopes
    }
}

impl RuntimeEnv {
    pub fn push_scope(&mut self) {
        self.push_scope_with_capacity(0);
    }

    pub(crate) fn push_scope_with_capacity(&mut self, binding_capacity: usize) {
        let mut scope = self
            .spare_scopes
            .pop()
            .unwrap_or_else(|| RuntimeScope::with_capacity(binding_capacity));
        scope.clear();
        scope.reserve_bindings(binding_capacity);
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            if let Some(mut scope) = self.scopes.pop() {
                scope.clear();
                self.spare_scopes.push(scope);
            }
        } else if let Some(scope) = self.scopes.last_mut() {
            scope.clear();
        }
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
        self.scopes.iter().rev().find_map(|scope| scope.get(local))
    }

    pub(crate) fn get_cloned(&self, local: RuntimeLocalDeclarationId) -> Option<RuntimeValue> {
        self.get(local).cloned()
    }

    pub(crate) fn set_record_field(
        &mut self,
        local: RuntimeLocalDeclarationId,
        field: RuntimeRecordFieldId,
        value: RuntimeValue,
    ) -> Result<(), RuntimeValue> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.binding_mut(local) {
                return set_runtime_record_field(&mut binding.value, field, value);
            }
        }
        Err(value)
    }

    pub fn bindings_snapshot(&self) -> Vec<RuntimeLocalBinding> {
        self.scopes
            .iter()
            .flat_map(|scope| scope.bindings.iter().cloned())
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
            bindings: Vec::with_capacity(binding_capacity),
        }
    }

    fn reserve_bindings(&mut self, binding_capacity: usize) {
        let additional = binding_capacity.saturating_sub(self.bindings.capacity());
        self.bindings.reserve(additional);
    }

    fn set(&mut self, local: RuntimeLocalDeclarationId, value: RuntimeValue) {
        if let Some(binding) = self.binding_mut(local) {
            binding.value = value;
        } else {
            self.bindings.push(RuntimeLocalBinding { local, value });
        }
    }

    fn set_ref(&mut self, local: RuntimeLocalDeclarationId, value: &RuntimeValue) {
        self.set(local, value.clone());
    }

    fn get(&self, local: RuntimeLocalDeclarationId) -> Option<&RuntimeValue> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.local == local)
            .map(|binding| &binding.value)
    }

    fn binding_mut(
        &mut self,
        local: RuntimeLocalDeclarationId,
    ) -> Option<&mut RuntimeLocalBinding> {
        self.bindings
            .iter_mut()
            .rev()
            .find(|binding| binding.local == local)
    }

    fn clear(&mut self) {
        self.bindings.clear();
    }

    fn replace_binding_values_ref(&mut self, bindings: &[RuntimeLocalBinding]) -> bool {
        if self.bindings.len() != bindings.len()
            || !self
                .bindings
                .iter()
                .zip(bindings)
                .all(|(current, next)| current.local == next.local)
        {
            return false;
        }
        for (current, next) in self.bindings.iter_mut().zip(bindings) {
            current.value = next.value.clone();
        }
        true
    }
}

fn set_runtime_record_field(
    target: &mut RuntimeValue,
    field: RuntimeRecordFieldId,
    value: RuntimeValue,
) -> Result<(), RuntimeValue> {
    let RuntimeValue::NominalRecord(record) = target else {
        return Err(value);
    };
    record.replace_field(field, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
    use crate::value::RuntimeNominalRecordValue;
    use std::num::NonZeroU32;

    fn local(ordinal: u32) -> RuntimeLocalDeclarationId {
        RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(ordinal).unwrap())
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
    fn field_assignment_uses_nominal_defining_order_identity() {
        let local = local(1);
        let field = RuntimeRecordFieldId::from_accepted_zero_based(1).unwrap();
        let mut env = RuntimeEnv::default();
        env.set(
            local,
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                RuntimeNominalTypeId::try_new("game.Pair").unwrap(),
                TypeLayoutHash::from_bytes([9; 32]),
                vec![
                    RuntimeValue::Bool(true),
                    RuntimeValue::String("old".to_owned()),
                ],
            )),
        );

        assert_eq!(
            env.set_record_field(local, field, RuntimeValue::String("new".to_owned())),
            Ok(())
        );
        let Some(RuntimeValue::NominalRecord(record)) = env.get(local) else {
            panic!("nominal record remains bound");
        };
        assert_eq!(
            record.field(field),
            Some(&RuntimeValue::String("new".to_owned()))
        );
    }
}
