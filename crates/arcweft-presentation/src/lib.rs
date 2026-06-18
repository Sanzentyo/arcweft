use arcweft_id::PublicId;
use core::marker::PhantomData;
use std::collections::HashMap;

pub mod gesture;
pub mod hit;
pub mod hover;
pub mod image;
pub mod input;
pub mod interaction;
pub mod layer;
pub mod replay;
pub mod router;
pub mod semantic;

/// Named lifetime owner for presentation values.
///
/// A scope is the data-model counterpart of an Arcweft lexical/lifetime scope:
/// handles registered in the scope are cleared when that scope exits unless
/// they were explicitly detached or moved into another scope.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PresentationScope {
    id: PublicId,
}

/// Slot key inside a presentation target.
///
/// Slots behave like a typed static `Option`: a target/slot pair may contain a
/// value, callers can read it without replacing it, and setting a new value
/// returns the previous value when one existed.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PresentationSlot {
    id: PublicId,
}

/// Render target that owns presentation slots.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PresentationTarget {
    id: PublicId,
}

/// Scope-bound handle returned by staging calls such as `bg(...)` and `show(...)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationHandle<T> {
    value: T,
    target: PresentationTarget,
    slot: PresentationSlot,
    scope: PresentationScope,
}

/// Typed reference to a presentation slot without changing the slot value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotRef<T> {
    target: PresentationTarget,
    slot: PresentationSlot,
    _ty: PhantomData<T>,
}

/// Static-option-like slot value used by Sans I/O presentation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotValue<T> {
    target: PresentationTarget,
    slot: PresentationSlot,
    value: Option<T>,
}

/// Scope-bound clear operation for a presentation slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClearPresentation<T> {
    target: PresentationTarget,
    slot: PresentationSlot,
    scope: PresentationScope,
    _ty: PhantomData<T>,
}

/// Background image currently registered in a background slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackgroundSurface {
    asset: PublicId,
}

/// Character stage object currently registered in a character slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterSurface {
    character: PublicId,
    expression: Option<PublicId>,
}

/// Typed registry for one presentation surface family.
///
/// The registry is Sans I/O: it records target/slot/scope ownership and returns
/// values to the caller, but it does not perform rendering, loading, or timing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationRegistry<T> {
    slots: HashMap<PresentationSlotKey, ScopedSlotValue<T>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PresentationSlotKey {
    target: PresentationTarget,
    slot: PresentationSlot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopedSlotValue<T> {
    scope: PresentationScope,
    value: T,
}

pub fn presentation_scope(name: &str) -> PresentationScope {
    PresentationScope::new(domain_id("scope", name))
}

pub fn presentation_target(name: &str) -> PresentationTarget {
    PresentationTarget::new(domain_id("target", name))
}

pub fn presentation_slot(name: &str) -> PresentationSlot {
    PresentationSlot::new(domain_id("slot", name))
}

pub fn asset(name: &str) -> PublicId {
    domain_id("asset", name)
}

pub fn bg(asset: PublicId, scope: PresentationScope) -> PresentationHandle<BackgroundSurface> {
    PresentationHandle::new(
        BackgroundSurface::new(asset),
        PresentationTarget::scene(),
        PresentationSlot::default_background(),
        scope,
    )
}

pub fn show_character(
    character: &PublicId,
    expression_name: &str,
    scope: PresentationScope,
) -> PresentationHandle<CharacterSurface> {
    PresentationHandle::new(
        CharacterSurface::new(character.clone(), Some(expression_name)),
        PresentationTarget::scene(),
        PresentationSlot::character(character),
        scope,
    )
}

pub fn bg_ref() -> SlotRef<BackgroundSurface> {
    SlotRef::new(
        PresentationTarget::scene(),
        PresentationSlot::default_background(),
    )
}

pub fn character_ref(character: &PublicId) -> SlotRef<CharacterSurface> {
    SlotRef::new(
        PresentationTarget::scene(),
        PresentationSlot::character(character),
    )
}

pub fn clear_bg(scope: PresentationScope) -> ClearPresentation<BackgroundSurface> {
    ClearPresentation::new(
        PresentationTarget::scene(),
        PresentationSlot::default_background(),
        scope,
    )
}

pub fn hide_character(
    character: &PublicId,
    scope: PresentationScope,
) -> ClearPresentation<CharacterSurface> {
    ClearPresentation::new(
        PresentationTarget::scene(),
        PresentationSlot::character(character),
        scope,
    )
}

fn expression(name: &str) -> PublicId {
    domain_id("expression", name)
}

fn domain_id(domain: &str, name: &str) -> PublicId {
    PublicId::try_new(format!("{domain}.{name}")).expect("domain helper requires a valid public id")
}

impl PresentationScope {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub fn flow() -> Self {
        presentation_scope("flow")
    }

    pub fn line() -> Self {
        presentation_scope("line")
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }
}

impl PresentationTarget {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub fn scene() -> Self {
        presentation_target("scene")
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }
}

impl PresentationSlot {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub fn default_background() -> Self {
        presentation_slot("background.default")
    }

    pub fn character(character: &PublicId) -> Self {
        let suffix = character
            .as_str()
            .strip_prefix("character.")
            .unwrap_or_else(|| character.as_str());
        presentation_slot(&format!("character.{suffix}.default"))
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }
}

impl BackgroundSurface {
    pub const fn new(asset: PublicId) -> Self {
        Self { asset }
    }

    pub const fn asset(&self) -> &PublicId {
        &self.asset
    }
}

impl CharacterSurface {
    pub fn new(character: PublicId, expression_name: Option<&str>) -> Self {
        Self {
            character,
            expression: expression_name.map(expression),
        }
    }

    pub const fn character(&self) -> &PublicId {
        &self.character
    }

    pub const fn expression(&self) -> Option<&PublicId> {
        self.expression.as_ref()
    }
}

impl<T> PresentationHandle<T> {
    pub const fn new(
        value: T,
        target: PresentationTarget,
        slot: PresentationSlot,
        scope: PresentationScope,
    ) -> Self {
        Self {
            value,
            target,
            slot,
            scope,
        }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn target(&self) -> &PresentationTarget {
        &self.target
    }

    pub const fn slot(&self) -> &PresentationSlot {
        &self.slot
    }

    pub const fn scope(&self) -> &PresentationScope {
        &self.scope
    }

    pub fn slot_ref(&self) -> SlotRef<T> {
        SlotRef::new(self.target.clone(), self.slot.clone())
    }
}

impl<T> SlotRef<T> {
    pub const fn new(target: PresentationTarget, slot: PresentationSlot) -> Self {
        Self {
            target,
            slot,
            _ty: PhantomData,
        }
    }

    pub const fn target(&self) -> &PresentationTarget {
        &self.target
    }

    pub const fn slot(&self) -> &PresentationSlot {
        &self.slot
    }
}

impl<T> SlotValue<T> {
    pub const fn empty(target: PresentationTarget, slot: PresentationSlot) -> Self {
        Self {
            target,
            slot,
            value: None,
        }
    }

    pub fn set(&mut self, handle: PresentationHandle<T>) -> Option<T> {
        let previous = self.value.take();
        self.target = handle.target;
        self.slot = handle.slot;
        self.value = Some(handle.value);
        previous
    }

    pub fn clear(&mut self) -> Option<T> {
        self.value.take()
    }

    pub const fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn slot_ref(&self) -> SlotRef<T> {
        SlotRef::new(self.target.clone(), self.slot.clone())
    }
}

impl<T> ClearPresentation<T> {
    pub const fn new(
        target: PresentationTarget,
        slot: PresentationSlot,
        scope: PresentationScope,
    ) -> Self {
        Self {
            target,
            slot,
            scope,
            _ty: PhantomData,
        }
    }

    pub const fn target(&self) -> &PresentationTarget {
        &self.target
    }

    pub const fn slot(&self) -> &PresentationSlot {
        &self.slot
    }

    pub const fn scope(&self) -> &PresentationScope {
        &self.scope
    }
}

impl<T> PresentationRegistry<T> {
    pub fn set(&mut self, handle: PresentationHandle<T>) -> Option<T> {
        let key = PresentationSlotKey {
            target: handle.target,
            slot: handle.slot,
        };
        self.slots
            .insert(
                key,
                ScopedSlotValue {
                    scope: handle.scope,
                    value: handle.value,
                },
            )
            .map(|previous| previous.value)
    }

    pub fn get(&self, slot: &SlotRef<T>) -> Option<&T> {
        self.slots
            .get(&PresentationSlotKey {
                target: slot.target.clone(),
                slot: slot.slot.clone(),
            })
            .map(|entry| &entry.value)
    }

    pub fn clear(&mut self, clear: &ClearPresentation<T>) -> Option<T> {
        self.slots
            .remove(&PresentationSlotKey {
                target: clear.target.clone(),
                slot: clear.slot.clone(),
            })
            .map(|entry| entry.value)
    }

    pub fn exit_scope(&mut self, scope: &PresentationScope) -> Vec<T> {
        let keys = self
            .slots
            .iter()
            .filter(|(_, value)| &value.scope == scope)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| self.slots.remove(&key).map(|entry| entry.value))
            .collect()
    }
}

impl<T> Default for PresentationRegistry<T> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests;
