use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{
    AwbcFrameLayout, AwbcFrameSlot, AwbcFrameSlotRole, AwbcRegisterId, AwbcScopeDefinition,
    AwbcScopeId, AwbcStringId, AwbcTypeId,
};
use arcweft_core::runtime_id::RuntimeLocalDeclarationId;
use arcweft_core::scope::RuntimeScopeIdentity;
use std::collections::BTreeMap;

/// Stable frame slot key. Accepted local declarations, rather than source
/// spellings, own lexical frame slots.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameSlotKey {
    Local(RuntimeLocalDeclarationId),
    Temp(u32),
    RootTemp(u32),
    ReturnValue(u32),
    RuntimeState(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCaptureSlot {
    pub local: RuntimeLocalDeclarationId,
    pub register: AwbcRegisterId,
}

/// Function-local frame allocator.
#[derive(Clone, Debug)]
pub struct FrameBuilder {
    slots: Vec<AwbcFrameSlot>,
    by_key: BTreeMap<FrameSlotKey, AwbcRegisterId>,
    temp_counter: u32,
    runtime_state_counter: u32,
    scopes: Vec<AwbcScopeDefinition>,
    active_scopes: Vec<AwbcScopeId>,
    max_scope_depth: u32,
}

impl FrameBuilder {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            by_key: BTreeMap::new(),
            temp_counter: 0,
            runtime_state_counter: 0,
            scopes: Vec::new(),
            active_scopes: Vec::new(),
            max_scope_depth: 0,
        }
    }

    pub fn slot(
        &mut self,
        key: FrameSlotKey,
        ty: AwbcTypeId,
        role: AwbcFrameSlotRole,
    ) -> AwbcRegisterId {
        self.slot_at_scope_depth(key, ty, role, self.scope_depth())
    }

    fn slot_at_scope_depth(
        &mut self,
        key: FrameSlotKey,
        ty: AwbcTypeId,
        role: AwbcFrameSlotRole,
        scope_depth: u32,
    ) -> AwbcRegisterId {
        if let Some(register) = self.by_key.get(&key).copied() {
            return register;
        }
        let register = AwbcRegisterId(table_index(self.slots.len()));
        self.slots.push(AwbcFrameSlot {
            name: None,
            ty,
            role,
            scope_depth,
        });
        self.by_key.insert(key, register);
        register
    }

    pub fn local(&mut self, local: RuntimeLocalDeclarationId, ty: AwbcTypeId) -> AwbcRegisterId {
        self.slot(FrameSlotKey::Local(local), ty, AwbcFrameSlotRole::Local)
    }

    pub fn parameter(
        &mut self,
        local: RuntimeLocalDeclarationId,
        ty: AwbcTypeId,
    ) -> AwbcRegisterId {
        self.slot(FrameSlotKey::Local(local), ty, AwbcFrameSlotRole::Parameter)
    }

    pub fn named_parameter(
        &mut self,
        local: RuntimeLocalDeclarationId,
        ty: AwbcTypeId,
        name: AwbcStringId,
    ) -> AwbcRegisterId {
        let register = self.parameter(local, ty);
        self.slots[register.index()].name = Some(name);
        register
    }

    pub fn temp(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::Temp(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot(key, ty, AwbcFrameSlotRole::Temporary)
    }

    pub fn root_temp(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::RootTemp(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot_at_scope_depth(key, ty, AwbcFrameSlotRole::Temporary, 0)
    }

    /// Keeps a scoped expression result alive across exactly one lexical exit.
    pub fn parent_temp(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::Temp(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        let depth = self
            .scope_depth()
            .checked_sub(1)
            .expect("scope result has a parent frame");
        self.slot_at_scope_depth(key, ty, AwbcFrameSlotRole::Temporary, depth)
    }

    pub fn return_value(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::ReturnValue(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot_at_scope_depth(key, ty, AwbcFrameSlotRole::ReturnValue, 0)
    }

    pub fn runtime_state(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::RuntimeState(self.runtime_state_counter);
        self.runtime_state_counter = self.runtime_state_counter.saturating_add(1);
        self.slot(key, ty, AwbcFrameSlotRole::RuntimeState)
    }

    pub fn enter_scope(&mut self) -> AwbcScopeId {
        self.enter_scope_with_identity(RuntimeScopeIdentity::Anonymous)
    }

    pub fn enter_scope_with_identity(&mut self, identity: RuntimeScopeIdentity) -> AwbcScopeId {
        let scope = AwbcScopeId(table_index(self.scopes.len()));
        self.scopes.push(AwbcScopeDefinition {
            parent: self.active_scopes.last().copied(),
            identity,
        });
        self.active_scopes.push(scope);
        self.max_scope_depth = self.max_scope_depth.max(self.scope_depth());
        scope
    }

    pub fn exit_scope(&mut self) {
        self.active_scopes.pop();
    }

    pub fn exit_all_scopes(&mut self) {
        self.active_scopes.clear();
    }

    pub fn scope_depth(&self) -> u32 {
        table_index(self.active_scopes.len())
    }

    pub fn scope_checkpoint(&self) -> Vec<AwbcScopeId> {
        self.active_scopes.clone()
    }

    pub fn restore_scopes_after_branch(&mut self, scopes: Vec<AwbcScopeId>) {
        self.active_scopes = scopes;
        self.max_scope_depth = self.max_scope_depth.max(self.scope_depth());
    }

    pub fn active_scope(&self) -> Option<AwbcScopeId> {
        self.active_scopes.last().copied()
    }

    pub fn scope_ids_for_exit_to_depth(
        &self,
        depth: u32,
    ) -> impl Iterator<Item = AwbcScopeId> + '_ {
        self.active_scopes
            .iter()
            .skip(depth as usize)
            .rev()
            .copied()
    }

    pub fn active_scope_ids_for_exit(&self) -> Vec<AwbcScopeId> {
        self.scope_ids_for_exit_to_depth(0).collect()
    }

    pub fn register_for_local(&self, local: RuntimeLocalDeclarationId) -> Option<AwbcRegisterId> {
        self.by_key.get(&FrameSlotKey::Local(local)).copied()
    }

    pub fn capture_slots(&self) -> Vec<FrameCaptureSlot> {
        self.by_key
            .iter()
            .filter_map(|(key, register)| match key {
                FrameSlotKey::Local(local) => Some(FrameCaptureSlot {
                    local: *local,
                    register: *register,
                }),
                FrameSlotKey::Temp(_)
                | FrameSlotKey::RootTemp(_)
                | FrameSlotKey::ReturnValue(_)
                | FrameSlotKey::RuntimeState(_) => None,
            })
            .collect()
    }

    pub fn finish(self) -> AwbcFrameLayout {
        AwbcFrameLayout {
            slots: self.slots,
            scopes: self.scopes,
            max_scope_depth: self.max_scope_depth,
        }
    }
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_definitions_distinguish_siblings_and_restore_exact_parent_stack() {
        let mut frame = FrameBuilder::new();
        let outer = frame.enter_scope_with_identity(RuntimeScopeIdentity::Named(
            arcweft_id::DeclarationName::try_new("outer").unwrap(),
        ));
        let checkpoint = frame.scope_checkpoint();
        let first = frame.enter_scope();
        frame.exit_all_scopes();
        frame.restore_scopes_after_branch(checkpoint);
        let second = frame.enter_scope();
        assert_ne!(first, second);
        assert_eq!(frame.active_scope_ids_for_exit(), [second, outer]);
        let layout = frame.finish();
        assert_eq!(layout.scopes[first.index()].parent, Some(outer));
        assert_eq!(layout.scopes[second.index()].parent, Some(outer));
        assert_eq!(layout.max_scope_depth, 2);
    }
}
