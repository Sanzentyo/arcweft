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
    Local { name: String, scope_depth: u32 },
    Temp(u32),
    RootTemp(u32),
    ReturnValue(u32),
    RuntimeState(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameCaptureSlot {
    pub name: String,
    pub name_id: AwbcStringId,
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
        name: Option<AwbcStringId>,
        ty: AwbcTypeId,
        role: AwbcFrameSlotRole,
    ) -> AwbcRegisterId {
        self.slot_at_scope_depth(key, name, ty, role, self.scope_depth)
    }

    fn slot_at_scope_depth(
        &mut self,
        key: FrameSlotKey,
        name: Option<AwbcStringId>,
        ty: AwbcTypeId,
        role: AwbcFrameSlotRole,
        scope_depth: u32,
    ) -> AwbcRegisterId {
        if let Some(register) = self.by_key.get(&key).copied() {
            return register;
        }
        let register = AwbcRegisterId(table_index(self.slots.len()));
        self.slots.push(AwbcFrameSlot {
            name,
            ty,
            role,
            scope_depth,
        });
        self.by_key.insert(key, register);
        register
    }

    pub fn local(&mut self, name: &str, name_id: AwbcStringId, ty: AwbcTypeId) -> AwbcRegisterId {
        self.slot(
            FrameSlotKey::Local {
                name: name.to_owned(),
                scope_depth: self.scope_depth,
            },
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
            FrameSlotKey::Local {
                name: name.to_owned(),
                scope_depth: self.scope_depth,
            },
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

    pub fn root_temp(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::RootTemp(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot_at_scope_depth(key, None, ty, AwbcFrameSlotRole::Temporary, 0)
    }

    pub fn return_value(&mut self, ty: AwbcTypeId) -> AwbcRegisterId {
        let key = FrameSlotKey::ReturnValue(self.temp_counter);
        self.temp_counter = self.temp_counter.saturating_add(1);
        self.slot_at_scope_depth(key, None, ty, AwbcFrameSlotRole::ReturnValue, 0)
    }

    pub fn runtime_state(
        &mut self,
        name: &str,
        name_id: AwbcStringId,
        ty: AwbcTypeId,
    ) -> AwbcRegisterId {
        self.slot(
            FrameSlotKey::RuntimeState(name.to_owned()),
            Some(name_id),
            ty,
            AwbcFrameSlotRole::RuntimeState,
        )
    }

    pub fn next_runtime_state_name(&mut self, prefix: &str) -> String {
        let ordinal = self.runtime_state_counter;
        self.runtime_state_counter = self.runtime_state_counter.saturating_add(1);
        format!("{prefix}.{ordinal}")
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

    pub fn register_for_local(&self, name: &str) -> Option<AwbcRegisterId> {
        (0..=self.scope_depth).rev().find_map(|scope_depth| {
            self.by_key
                .get(&FrameSlotKey::Local {
                    name: name.to_owned(),
                    scope_depth,
                })
                .copied()
        })
    }

    pub fn capture_slots(&self) -> Vec<FrameCaptureSlot> {
        let mut visible = BTreeMap::<String, (u32, FrameCaptureSlot)>::new();
        for (key, register) in &self.by_key {
            let FrameSlotKey::Local { name, scope_depth } = key else {
                continue;
            };
            if *scope_depth > self.scope_depth {
                continue;
            }
            let Some(name_id) = self.slots.get(register.index()).and_then(|slot| slot.name) else {
                continue;
            };
            let capture = FrameCaptureSlot {
                name: name.clone(),
                name_id,
                register: *register,
            };
            match visible.entry(name.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((*scope_depth, capture));
                }
                std::collections::btree_map::Entry::Occupied(mut entry)
                    if *scope_depth > entry.get().0 =>
                {
                    entry.insert((*scope_depth, capture));
                }
                std::collections::btree_map::Entry::Occupied(_) => {}
            }
        }
        visible.into_values().map(|(_, capture)| capture).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_lookup_and_capture_select_the_innermost_lexical_slot() {
        let mut frame = FrameBuilder::new();
        let ty = AwbcTypeId(0);
        let outer = frame.local("value", AwbcStringId(0), ty);

        let _scope = frame.enter_scope();
        let inner = frame.local("value", AwbcStringId(1), ty);

        assert_ne!(outer, inner);
        assert_eq!(frame.register_for_local("value"), Some(inner));
        assert_eq!(
            frame.capture_slots(),
            vec![FrameCaptureSlot {
                name: "value".to_owned(),
                name_id: AwbcStringId(1),
                register: inner,
            }]
        );

        frame.exit_scope();
        assert_eq!(frame.register_for_local("value"), Some(outer));
        assert_eq!(
            frame.capture_slots(),
            vec![FrameCaptureSlot {
                name: "value".to_owned(),
                name_id: AwbcStringId(0),
                register: outer,
            }]
        );
    }
}
