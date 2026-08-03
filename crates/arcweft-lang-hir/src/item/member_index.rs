//! Immutable module index for per-item declaration-member arenas.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::identity::{HirModuleId, ItemId};

use super::retained::{
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberResolveError,
};
use super::{HirItem, HirItemInvariantError};

/// Immutable module-owned index of every staged per-item member arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationMemberIndex {
    module: HirModuleId,
    arenas: BTreeMap<ItemId, HirDeclarationMemberArena>,
}

impl HirDeclarationMemberIndex {
    pub const fn module(&self) -> HirModuleId {
        self.module
    }

    pub fn arenas(&self) -> &BTreeMap<ItemId, HirDeclarationMemberArena> {
        &self.arenas
    }

    pub fn arena(&self, owner: ItemId) -> Option<&HirDeclarationMemberArena> {
        self.arenas.get(&owner)
    }

    pub fn resolve(
        &self,
        id: HirDeclarationMemberId,
    ) -> Result<&HirDeclarationMember, HirDeclarationMemberIndexResolveError> {
        let arena = self
            .arenas
            .get(&id.item())
            .ok_or(HirDeclarationMemberIndexResolveError::UnknownOwner { owner: id.item() })?;
        arena.resolve(id).map_err(|error| match error {
            HirDeclarationMemberResolveError::UnknownOrdinal(ordinal) => {
                HirDeclarationMemberIndexResolveError::UnknownOrdinal {
                    owner: id.item(),
                    ordinal,
                }
            }
            HirDeclarationMemberResolveError::ForeignOwner { expected, actual } => {
                HirDeclarationMemberIndexResolveError::CorruptOwner { expected, actual }
            }
        })
    }
}

/// Resolve failure against a frozen module member index.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationMemberIndexResolveError {
    #[error("module member index has no arena for item {owner:?}")]
    UnknownOwner { owner: ItemId },
    #[error("item {owner:?} has no declaration member at ordinal {ordinal}")]
    UnknownOrdinal { owner: ItemId, ordinal: u32 },
    #[error("member arena owner is corrupt: expected {expected:?}, got {actual:?}")]
    CorruptOwner { expected: ItemId, actual: ItemId },
}

/// Transaction-local builder that publishes only complete item/member pairs.
#[derive(Debug)]
pub(crate) struct HirDeclarationMemberIndexBuilder {
    module: HirModuleId,
    staged: BTreeMap<ItemId, HirDeclarationMemberArena>,
}

impl HirDeclarationMemberIndexBuilder {
    pub(crate) fn new(module: HirModuleId) -> Self {
        Self {
            module,
            staged: BTreeMap::new(),
        }
    }

    pub(crate) fn stage(
        &mut self,
        owner: ItemId,
        item: &HirItem,
        arena: HirDeclarationMemberArena,
    ) -> Result<(), HirItemInvariantError> {
        if owner.module() != self.module {
            return Err(HirItemInvariantError::ForeignChild {
                expected: self.module,
                actual: owner.module(),
            });
        }
        if item.scope().module() != self.module {
            return Err(HirItemInvariantError::ForeignChild {
                expected: self.module,
                actual: item.scope().module(),
            });
        }
        if arena.owner() != owner {
            return Err(HirItemInvariantError::MemberArenaOwnerMismatch {
                expected: owner,
                actual: arena.owner(),
            });
        }
        if arena.family() != item.family() {
            return Err(HirItemInvariantError::MemberArenaFamilyMismatch {
                owner,
                item_family: item.family(),
                arena_family: arena.family(),
            });
        }
        if item.members().is_empty() {
            return Err(HirItemInvariantError::MemberArenaNotRequired { owner });
        }
        if item.members().len() != arena.members().len()
            || item
                .members()
                .iter()
                .zip(arena.members())
                .any(|(expected, actual)| *expected != actual.id())
        {
            return Err(HirItemInvariantError::MemberArenaItemOrderMismatch { owner });
        }
        if item.family() == super::HirItemFamily::Layer
            && !item.is_poisoned()
            && arena
                .members()
                .iter()
                .any(HirDeclarationMember::is_poisoned)
        {
            return Err(HirItemInvariantError::InvalidPoisonState);
        }
        match self.staged.entry(owner) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(arena);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(HirItemInvariantError::DuplicateMemberArenaOwner { owner })
            }
        }
    }

    pub(crate) fn freeze(self) -> HirDeclarationMemberIndex {
        HirDeclarationMemberIndex {
            module: self.module,
            arenas: self.staged,
        }
    }
}
