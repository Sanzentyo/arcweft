//! Stable slots for types and metadata instantiated during a single walk.

use std::cell::{Cell, OnceCell};

use super::{InstantiatedRustTypeMetadata, RuntimeNominalGraphProjectionLimits, TypeKind};
use crate::types::GenericTypeParameterId;

pub(super) struct ProjectionStorage {
    pub(super) types: ProjectionSlots<TypeKind>,
    pub(super) parameters: ProjectionSlots<Box<[GenericTypeParameterId]>>,
    pub(super) rust_metadata: ProjectionSlots<InstantiatedRustTypeMetadata>,
}

impl ProjectionStorage {
    pub(super) fn new() -> Self {
        let hard = RuntimeNominalGraphProjectionLimits::PRODUCTION;
        Self {
            types: ProjectionSlots::new(usize::try_from(hard.max_type_nodes).expect("type budget")),
            parameters: ProjectionSlots::new(
                usize::try_from(hard.max_definitions).expect("definition budget"),
            ),
            rust_metadata: ProjectionSlots::new(
                usize::try_from(hard.max_definitions).expect("definition budget"),
            ),
        }
    }
}

type Page<T> = Box<[OnceCell<T>]>;

/// Each insertion is preceded by a type-node or definition charge. Pages are
/// allocated on demand, and their contents are dropped with the transaction.
pub(super) struct ProjectionSlots<T> {
    pages: OnceCell<Box<[OnceCell<Page<T>>]>>,
    next: Cell<usize>,
    capacity: usize,
}

const PAGE_LEN: usize = 32;

impl<T> ProjectionSlots<T> {
    const fn new(capacity: usize) -> Self {
        Self {
            pages: OnceCell::new(),
            next: Cell::new(0),
            capacity,
        }
    }

    pub(super) fn insert(&self, value: T) -> (usize, &T) {
        let index = self.next.get();
        let pages = self.pages.get_or_init(|| {
            std::iter::repeat_with(OnceCell::new)
                .take(self.capacity.div_ceil(PAGE_LEN))
                .collect()
        });
        let page = pages[index / PAGE_LEN].get_or_init(|| {
            std::iter::repeat_with(OnceCell::new)
                .take(PAGE_LEN)
                .collect()
        });
        let stored = page[index % PAGE_LEN].get_or_init(|| value);
        self.next.set(index + 1);
        (index, stored)
    }

    pub(super) fn take(&mut self, index: usize) -> T {
        self.pages.get_mut().expect("page was allocated")[index / PAGE_LEN]
            .get_mut()
            .expect("page was allocated")[index % PAGE_LEN]
            .take()
            .expect("slot was filled")
    }
}
