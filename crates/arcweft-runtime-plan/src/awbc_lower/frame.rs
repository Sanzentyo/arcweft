use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{
    AwbcFrameLayout, AwbcFrameSlot, AwbcFrameSlotRole, AwbcRegisterId, AwbcScopeId, AwbcStringId,
    AwbcTypeId,
};
use std::collections::BTreeMap;

/// Stable frame slot key. Local names and generated temporaries share one
/// allocator so pattern bindings, captures and call destinations cannot alias by
/// accident.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameSlotKey {
    Local(String),
    Temp(u32),
    ReturnValue(u32),
    RuntimeState(&'static str),
}

/// Function-local frame allocator.
#[derive(Clone, Debug)]
pub struct FrameBuilder {
    slots: Vec<AwbcFrameSlot>,
    by_key: BTreeMap<FrameSlotKey, AwbcRegisterId>,
    temp_counter: u32,
    scope_depth: u32,
    max_scope_depth: u32,
}

impl FrameBuilder {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            by_key: BTreeMap::new(),
            temp_counter: 0,
            scope_depth: 0,
            max_scope_depth: 0,
        }
    }

    pub fn slot(
        &mut self,
        key: FrameSlotKey,
        name: Option<AwbcStringId>,
        ty: AwbcTypeId,
        role: AwbcFrameSlotRole,
    ) -> AwbcRegisterId {
        if let Some(register) = self.by_key.get(&key).copied() {
            return register;
        }
        let register = AwbcRegisterId(table_index(self.slots.len()));
        self.slots.push(AwbcFrameSlot {
            name,
            ty,
            role,
            scope_depth: self.scope_depth,
        });
        self.by_key.insert(key, register);
        register
    }

    pub fn local(&mut self, name: &str, name_id: AwbcStringId, ty: AwbcTypeId) -> AwbcRegisterId {
        self.slot(
            FrameSlotKey::Local(name.to_owned()),
            Some(name_id),
            ty,
            AwbcFrameSlotRole::Local,
        )
    }

    pub fn parameter(
        &mut self,
        name: &str,
        name_id: AwbcStringId,
        ty: AwbcTypeId,
    ) -> AwbcRegisterId {
        self.slot(
            FrameSlotKey::Local(name.to_owned()),
            Some(name_id),
            ty,
            AwbcFrameSlotRole::Parameter,
        )
    }

    pub fn temp(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::Temp(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot(key, None, ty, AwbcFrameSlotRole::Temporary)
    }

    pub fn return_value(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::ReturnValue(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot(key, None, ty, AwbcFrameSlotRole::ReturnValue)
    }

    pub fn runtime_state(
        &mut self,
        name: &'static str,
        name_id: AwbcStringId,
        ty: AwbcTypeId,
    ) -> AwbcRegisterId {
        self.slot(
            FrameSlotKey::RuntimeState(name),
            Some(name_id),
            ty,
            AwbcFrameSlotRole::RuntimeState,
        )
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

    pub fn register_for_local(&self, name: &str) -> Option<AwbcRegisterId> {
        self.by_key
            .get(&FrameSlotKey::Local(name.to_owned()))
            .copied()
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
