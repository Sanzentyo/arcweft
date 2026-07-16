//! Safe generational View entity storage for stateful views.

use crate::{ViewError, ViewRegistryId};
use core::{any::Any, marker::PhantomData, num::NonZeroU32};

/// Untyped generational entity handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RawEntity {
    index: u32,
    generation: NonZeroU32,
}

/// Typed generational handle for view-local state.
#[repr(transparent)]
pub struct Entity<T> {
    raw: RawEntity,
    marker: PhantomData<fn() -> T>,
}

/// Dirty flags tracked on stateful view entities.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirtyFlags(u8);

#[derive(Debug)]
struct EntitySlot {
    generation: NonZeroU32,
    state: Option<Box<dyn Any>>,
    view: Option<ViewRegistryId>,
    dirty: DirtyFlags,
    queued: bool,
}

/// Reusable store for stateful View view entities.
#[derive(Debug, Default)]
pub struct EntityStore {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
}

impl RawEntity {
    pub const fn new(index: u32, generation: NonZeroU32) -> Self {
        Self { index, generation }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn generation(self) -> NonZeroU32 {
        self.generation
    }
}

impl<T> Copy for Entity<T> {}

impl<T> Clone for Entity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> core::fmt::Debug for Entity<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Entity").field(&self.raw).finish()
    }
}

impl<T> Eq for Entity<T> {}

impl<T> PartialEq for Entity<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> Entity<T> {
    pub const fn from_raw(raw: RawEntity) -> Self {
        Self {
            raw,
            marker: PhantomData,
        }
    }

    pub const fn raw(self) -> RawEntity {
        self.raw
    }
}

impl DirtyFlags {
    pub const NONE: Self = Self(0);
    pub const STATE: Self = Self(1 << 0);
    pub const FRAGMENT: Self = Self(1 << 1);
    pub const LAYOUT: Self = Self(1 << 2);
    pub const SEMANTICS: Self = Self(1 << 3);
    pub const PAINT: Self = Self(1 << 4);

    pub const fn all() -> Self {
        Self(Self::STATE.0 | Self::FRAGMENT.0 | Self::LAYOUT.0 | Self::SEMANTICS.0 | Self::PAINT.0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    pub fn insert(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    pub fn remove(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

impl EntitySlot {
    fn occupied<T: 'static>(state: T, view: Option<ViewRegistryId>) -> Self {
        Self {
            generation: NonZeroU32::MIN,
            state: Some(Box::new(state)),
            view,
            dirty: DirtyFlags::all(),
            queued: true,
        }
    }

    fn next_generation(&mut self) -> Result<NonZeroU32, ViewError> {
        let next = self
            .generation
            .get()
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .ok_or(ViewError::CapacityExceeded)?;
        self.generation = next;
        Ok(next)
    }
}

impl EntityStore {
    pub fn insert<T: 'static>(
        &mut self,
        state: T,
        view: Option<ViewRegistryId>,
    ) -> Result<Entity<T>, ViewError> {
        if let Some(index) = self.free.pop() {
            let slot = self
                .slots
                .get_mut(index as usize)
                .ok_or(ViewError::CapacityExceeded)?;
            let generation = slot.next_generation()?;
            slot.state = Some(Box::new(state));
            slot.view = view;
            slot.dirty = DirtyFlags::all();
            slot.queued = true;
            return Ok(Entity::from_raw(RawEntity::new(index, generation)));
        }

        let index = u32::try_from(self.slots.len()).map_err(|_| ViewError::CapacityExceeded)?;
        self.slots.push(EntitySlot::occupied(state, view));
        Ok(Entity::from_raw(RawEntity::new(index, NonZeroU32::MIN)))
    }

    pub fn get<T: 'static>(&self, entity: Entity<T>) -> Option<&T> {
        self.valid_slot(entity.raw)
            .and_then(|slot| slot.state.as_ref()?.downcast_ref())
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity<T>) -> Option<&mut T> {
        self.valid_slot_mut(entity.raw)
            .and_then(|slot| slot.state.as_mut()?.downcast_mut())
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity<T>) -> Result<T, ViewError> {
        let slot = self
            .valid_slot_mut(entity.raw)
            .ok_or(ViewError::StaleEntity(entity.raw))?;
        if !slot
            .state
            .as_ref()
            .ok_or(ViewError::StaleEntity(entity.raw))?
            .is::<T>()
        {
            return Err(ViewError::EntityTypeMismatch(entity.raw));
        }
        let boxed = slot
            .state
            .take()
            .ok_or(ViewError::StaleEntity(entity.raw))?;
        let state = boxed
            .downcast::<T>()
            .map_err(|_| ViewError::EntityTypeMismatch(entity.raw))?;
        slot.view = None;
        slot.dirty = DirtyFlags::NONE;
        slot.queued = false;
        self.free.push(entity.raw.index);
        Ok(*state)
    }

    pub fn view<T>(&self, entity: Entity<T>) -> Option<ViewRegistryId> {
        self.valid_slot(entity.raw).and_then(|slot| slot.view)
    }

    pub fn dirty<T>(&self, entity: Entity<T>) -> Option<DirtyFlags> {
        self.valid_slot(entity.raw).map(|slot| slot.dirty)
    }

    pub fn mark_dirty<T>(&mut self, entity: Entity<T>, flag: DirtyFlags) -> Result<(), ViewError> {
        let slot = self
            .valid_slot_mut(entity.raw)
            .ok_or(ViewError::StaleEntity(entity.raw))?;
        slot.dirty.insert(flag);
        slot.queued = true;
        Ok(())
    }

    pub fn clear_dirty<T>(&mut self, entity: Entity<T>, flag: DirtyFlags) -> Result<(), ViewError> {
        let slot = self
            .valid_slot_mut(entity.raw)
            .ok_or(ViewError::StaleEntity(entity.raw))?;
        slot.dirty.remove(flag);
        slot.queued = slot.dirty != DirtyFlags::NONE;
        Ok(())
    }

    pub fn is_queued<T>(&self, entity: Entity<T>) -> Option<bool> {
        self.valid_slot(entity.raw).map(|slot| slot.queued)
    }

    fn valid_slot(&self, raw: RawEntity) -> Option<&EntitySlot> {
        let slot = self.slots.get(raw.index as usize)?;
        (slot.generation == raw.generation)
            .then_some(slot)
            .filter(|slot| slot.state.is_some())
    }

    fn valid_slot_mut(&mut self, raw: RawEntity) -> Option<&mut EntitySlot> {
        let slot = self.slots.get_mut(raw.index as usize)?;
        (slot.generation == raw.generation && slot.state.is_some()).then_some(slot)
    }
}
