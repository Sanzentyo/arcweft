use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{
    AwbcFrameLayout, AwbcFrameSlot, AwbcFrameSlotRole, AwbcRegisterId, AwbcScopeId, AwbcStringId,
    AwbcTypeId,
};
use arcweft_core::runtime_id::RuntimeLocalDeclarationId;
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
    scope_depth: u32,
    max_scope_depth: u32,
}

impl FrameBuilder {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            by_key: BTreeMap::new(),
            temp_counter: 0,
            runtime_state_counter: 0,
            scope_depth: 0,
            max_scope_depth: 0,
        }
    }

    pub fn slot(
        &mut self,
        key: FrameSlotKey,
        ty: AwbcTypeId,
        role: AwbcFrameSlotRole,
    ) -> AwbcRegisterId {
        self.slot_at_scope_depth(key, ty, role, self.scope_depth)
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
        let scope = AwbcScopeId(self.scope_depth);
        self.scope_depth = self.scope_depth.saturating_add(1);
        self.max_scope_depth = self.max_scope_depth.max(self.scope_depth);
        scope
    }

    pub fn exit_scope(&mut self) {
        self.scope_depth = self.scope_depth.saturating_sub(1);
    }

    pub fn exit_all_scopes(&mut self) {
        self.scope_depth = 0;
    }

    pub const fn scope_depth(&self) -> u32 {
        self.scope_depth
    }

    pub fn restore_scope_depth_after_branch(&mut self, depth: u32) {
        self.scope_depth = depth;
        self.max_scope_depth = self.max_scope_depth.max(depth);
    }

    pub fn active_scope_ids_for_exit(&self) -> Vec<AwbcScopeId> {
        (0..self.scope_depth).rev().map(AwbcScopeId).collect()
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
            max_scope_depth: self.max_scope_depth,
        }
    }
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}
