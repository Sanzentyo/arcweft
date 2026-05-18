//! Typed symbolic references and handle markers.
//!
//! These wrappers distinguish Arcweft entity references, owned handles, weak
//! handles, and borrow-like views without introducing host I/O or ownership
//! backends into low-level crates.

use arcweft_id::EntityId;
use core::marker::PhantomData;

/// Internal typed entity ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id<T> {
    id: EntityId,
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Creates a typed ID from a validated Arcweft entity ID.
    pub const fn new(id: EntityId) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }

    /// Underlying untyped entity ID.
    pub const fn entity_id(&self) -> &EntityId {
        &self.id
    }
}

/// Symbolic non-null reference to an Arcweft entity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ref<T>(Id<T>);

impl<T> Ref<T> {
    /// Creates a symbolic entity reference.
    pub const fn new(id: Id<T>) -> Self {
        Self(id)
    }

    /// Typed ID carried by this reference.
    pub const fn id(&self) -> &Id<T> {
        &self.0
    }
}

/// Owned runtime handle whose lifetime is tracked by the owning system.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Handle<T> {
    id: Id<T>,
}

impl<T> Handle<T> {
    /// Creates an owned handle from a typed ID.
    pub const fn new(id: Id<T>) -> Self {
        Self { id }
    }

    /// Handle ID.
    pub const fn id(&self) -> &Id<T> {
        &self.id
    }
}

/// Weak handle that can fail to upgrade once the owned handle is gone.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WeakHandle<T> {
    id: Id<T>,
}

impl<T> WeakHandle<T> {
    /// Creates a weak handle marker.
    pub const fn new(id: Id<T>) -> Self {
        Self { id }
    }
}

/// Lease marker for resources bound to a runtime lifetime/scope.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Lease<T> {
    id: Id<T>,
}

impl<T> Lease<T> {
    /// Creates a scoped lease marker.
    pub const fn new(id: Id<T>) -> Self {
        Self { id }
    }
}

/// Borrow-like typed view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Borrow<'a, T> {
    value: &'a T,
}

impl<'a, T> Borrow<'a, T> {
    /// Wraps a Rust borrow as an Arcweft borrow marker.
    pub const fn new(value: &'a T) -> Self {
        Self { value }
    }

    /// Underlying borrow.
    pub const fn get(self) -> &'a T {
        self.value
    }
}

/// Borrowed contiguous view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slice<'a, T> {
    value: &'a [T],
}

impl<'a, T> Slice<'a, T> {
    /// Wraps a borrowed slice.
    pub const fn new(value: &'a [T]) -> Self {
        Self { value }
    }

    /// Underlying slice.
    pub const fn as_slice(self) -> &'a [T] {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::{Borrow, Id, Ref, Slice};
    use arcweft_id::EntityId;

    struct Flow;

    #[test]
    fn typed_ref_keeps_entity_id() {
        let id = Id::<Flow>::new(EntityId::try_new("runtime.flow.1").expect("id"));
        let reference = Ref::new(id);
        assert_eq!(reference.id().entity_id().as_str(), "runtime.flow.1");
    }

    #[test]
    fn borrowed_views_are_explicit() {
        let value = 7;
        let borrowed = Borrow::new(&value);
        assert_eq!(*borrowed.get(), 7);
        let items = [1, 2, 3];
        assert_eq!(Slice::new(&items).as_slice(), &[1, 2, 3]);
    }
}
