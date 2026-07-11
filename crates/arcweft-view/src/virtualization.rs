//! Deterministic range virtualization for retained View list mounts.
//!
//! This module owns already-resolved item extents, stable keys, mount
//! allocation, and exact save/observation records. It does not measure
//! content, evaluate View expressions, or perform platform I/O.

use crate::{
    ViewMountAllocator, ViewMountId,
    program::{ViewStableKey, ViewVirtualAxis},
};
use arcweft_id::PublicId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use thiserror::Error;

/// Authored Scroll target that owns a virtualized list's primary-axis offset.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewVirtualScrollTarget(String);

/// One finite source item after its primary-axis extent has been resolved.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualItem {
    pub key: ViewStableKey,
    pub extent_milli: u32,
}

/// Half-open logical item window selected for materialization.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualWindow {
    pub start: u32,
    pub end: u32,
}

/// Stable range record published for both materialized and retained-only items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualRange {
    pub index: u32,
    pub key: ViewStableKey,
    pub start_milli: u64,
    pub extent_milli: u32,
    pub materialized: bool,
}

/// Bounded page of range records for large Agent/debug queries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualRangePage {
    pub total_items: u32,
    pub start: u32,
    pub end: u32,
    pub items: Vec<ViewVirtualRange>,
}

/// Scroll position relative to a stable item.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualAnchor {
    pub key: ViewStableKey,
    /// Offset from the item start. It may equal the item extent when a
    /// zero-sized viewport is positioned at the end of the source.
    pub offset_within_item_milli: u32,
}

/// Exact save/load state for one mounted virtual list.
///
/// The full finite item inventory and absolute offset are authoritative. The
/// derived key anchor is stored as an integrity check; live source revisions
/// use `ViewVirtualList::replace_items` to preserve an anchor across reordering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualListSnapshot {
    pub mount: ViewMountId,
    pub scroll_target: ViewVirtualScrollTarget,
    pub axis: ViewVirtualAxis,
    pub viewport_extent_milli: u32,
    pub items: Vec<ViewVirtualItem>,
    pub absolute_offset_milli: u64,
    pub anchor: Option<ViewVirtualAnchor>,
}

/// Complete save state including the monotonic mount allocator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualizationSnapshot {
    pub next_mount_id: u64,
    pub mounts: Vec<ViewVirtualListSnapshot>,
}

/// Complete observation/capture table for one mounted virtual list.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewVirtualRangeTable {
    pub mount: ViewMountId,
    pub scroll_target: ViewVirtualScrollTarget,
    pub axis: ViewVirtualAxis,
    pub viewport_extent_milli: u32,
    pub offset_milli: u64,
    pub total_extent_milli: u64,
    pub materialized: ViewVirtualWindow,
    pub items: Vec<ViewVirtualRange>,
}

/// Exact finite-list virtualization state for one mounted View occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewVirtualList {
    mount: ViewMountId,
    scroll_target: ViewVirtualScrollTarget,
    axis: ViewVirtualAxis,
    viewport_extent_milli: u32,
    offset_milli: u64,
    total_extent_milli: u64,
    items: Vec<ViewVirtualItem>,
    starts_milli: Vec<u64>,
    indices: BTreeMap<ViewStableKey, u32>,
}

/// Independent virtual-list instances plus their monotonic mount allocator.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewVirtualizationRuntime {
    mount_allocator: ViewMountAllocator,
    mounts: BTreeMap<ViewMountId, ViewVirtualList>,
}

struct IndexedItems {
    starts_milli: Vec<u64>,
    indices: BTreeMap<ViewStableKey, u32>,
    total_extent_milli: u64,
}

/// Invalid virtual-list input or snapshot state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewVirtualizationError {
    #[error("virtual list item {index} has a zero primary-axis extent")]
    ZeroItemExtent { index: u32 },
    #[error("duplicate virtual list item key {key:?}")]
    DuplicateItemKey { key: ViewStableKey },
    #[error("virtual list contains more than u32::MAX items")]
    ItemCapacityExceeded,
    #[error("the View mount-id allocator is exhausted")]
    MountIdExhausted,
    #[error("the View mount-id allocator attempted to reuse live mount {mount:?}")]
    MountIdCollision { mount: ViewMountId },
    #[error("unknown View mount {mount:?}")]
    UnknownMount { mount: ViewMountId },
    #[error("virtual-list snapshot repeats View mount {mount:?}")]
    DuplicateSnapshotMount { mount: ViewMountId },
    #[error("virtual-list snapshot mount mismatch: saved {saved:?}, mounted {mounted:?}")]
    SnapshotMountMismatch {
        saved: ViewMountId,
        mounted: ViewMountId,
    },
    #[error(
        "virtual-list snapshot axis mismatch for {mount:?}: saved {saved:?}, mounted {mounted:?}"
    )]
    SnapshotAxisMismatch {
        mount: ViewMountId,
        saved: ViewVirtualAxis,
        mounted: ViewVirtualAxis,
    },
    #[error(
        "virtual-list snapshot Scroll target mismatch for {mount:?}: saved {saved:?}, mounted {mounted:?}"
    )]
    SnapshotScrollTargetMismatch {
        mount: ViewMountId,
        saved: ViewVirtualScrollTarget,
        mounted: ViewVirtualScrollTarget,
    },
    #[error(
        "virtual-list snapshot offset {saved} exceeds maximum {maximum} for View mount {mount:?}"
    )]
    SnapshotOffsetOutOfRange {
        mount: ViewMountId,
        saved: u64,
        maximum: u64,
    },
    #[error("virtual-list snapshot anchor is inconsistent for View mount {mount:?}")]
    SnapshotAnchorMismatch { mount: ViewMountId },
    #[error(
        "virtual-list snapshot next mount id {next_mount_id} is not newer than active mount {greatest_mount_id}"
    )]
    SnapshotMountAllocatorNotFresh {
        next_mount_id: u64,
        greatest_mount_id: u64,
    },
}

impl ViewVirtualScrollTarget {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<PublicId> for ViewVirtualScrollTarget {
    fn from(target: PublicId) -> Self {
        Self(target.as_str().to_owned())
    }
}

impl<'de> Deserialize<'de> for ViewVirtualScrollTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        PublicId::try_new(value)
            .map(Self::from)
            .map_err(serde::de::Error::custom)
    }
}

impl ViewVirtualItem {
    pub const fn new(key: ViewStableKey, extent_milli: u32) -> Self {
        Self { key, extent_milli }
    }
}

impl ViewVirtualWindow {
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn contains(self, index: u32) -> bool {
        self.start <= index && index < self.end
    }
}

impl ViewVirtualRange {
    pub fn end_milli(self) -> u64 {
        self.start_milli
            .saturating_add(u64::from(self.extent_milli))
    }
}

impl ViewVirtualList {
    fn new(
        mount: ViewMountId,
        scroll_target: ViewVirtualScrollTarget,
        axis: ViewVirtualAxis,
        viewport_extent_milli: u32,
        items: Vec<ViewVirtualItem>,
    ) -> Result<Self, ViewVirtualizationError> {
        let indexed = index_items(&items)?;
        Ok(Self {
            mount,
            scroll_target,
            axis,
            viewport_extent_milli,
            offset_milli: 0,
            total_extent_milli: indexed.total_extent_milli,
            items,
            starts_milli: indexed.starts_milli,
            indices: indexed.indices,
        })
    }

    pub const fn mount(&self) -> ViewMountId {
        self.mount
    }

    pub fn scroll_target(&self) -> &ViewVirtualScrollTarget {
        &self.scroll_target
    }

    pub const fn axis(&self) -> ViewVirtualAxis {
        self.axis
    }

    pub const fn viewport_extent_milli(&self) -> u32 {
        self.viewport_extent_milli
    }

    pub const fn offset_milli(&self) -> u64 {
        self.offset_milli
    }

    pub const fn total_extent_milli(&self) -> u64 {
        self.total_extent_milli
    }

    pub fn items(&self) -> &[ViewVirtualItem] {
        &self.items
    }

    pub fn max_offset_milli(&self) -> u64 {
        self.total_extent_milli
            .saturating_sub(u64::from(self.viewport_extent_milli))
    }

    pub fn set_viewport_extent_milli(&mut self, extent_milli: u32) {
        self.viewport_extent_milli = extent_milli;
        self.offset_milli = self.offset_milli.min(self.max_offset_milli());
    }

    pub fn scroll_to_milli(&mut self, offset_milli: u64) -> u64 {
        self.offset_milli = offset_milli.min(self.max_offset_milli());
        self.offset_milli
    }

    pub fn scroll_by_milli(&mut self, delta_milli: i64) -> u64 {
        let requested = if delta_milli.is_negative() {
            self.offset_milli.saturating_sub(delta_milli.unsigned_abs())
        } else {
            self.offset_milli
                .saturating_add(delta_milli.cast_unsigned())
        };
        self.scroll_to_milli(requested)
    }

    /// Replaces a live finite source atomically while retaining its key anchor.
    pub fn replace_items(
        &mut self,
        items: Vec<ViewVirtualItem>,
    ) -> Result<(), ViewVirtualizationError> {
        let anchor = self.anchor();
        let old_absolute = self.offset_milli;
        let mut replacement = Self::new(
            self.mount,
            self.scroll_target.clone(),
            self.axis,
            self.viewport_extent_milli,
            items,
        )?;
        replacement.offset_milli = replacement
            .offset_for_anchor(anchor)
            .unwrap_or(old_absolute)
            .min(replacement.max_offset_milli());
        *self = replacement;
        Ok(())
    }

    pub fn materialized_window(&self) -> ViewVirtualWindow {
        if self.viewport_extent_milli == 0 || self.items.is_empty() {
            return ViewVirtualWindow::default();
        }
        let viewport_end = self
            .offset_milli
            .saturating_add(u64::from(self.viewport_extent_milli));
        let start = self
            .starts_milli
            .partition_point(|start| *start <= self.offset_milli)
            .saturating_sub(1);
        let end = self
            .starts_milli
            .partition_point(|start| *start < viewport_end);
        ViewVirtualWindow {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(end).unwrap_or(u32::MAX),
        }
    }

    pub fn materialized_items(&self) -> &[ViewVirtualItem] {
        let window = self.materialized_window();
        let start = usize::try_from(window.start).unwrap_or(usize::MAX);
        let end = usize::try_from(window.end).unwrap_or(usize::MAX);
        &self.items[start..end]
    }

    /// Returns a bounded deterministic page, including off-window items.
    pub fn range_page(&self, start: u32, max_items: u32) -> ViewVirtualRangePage {
        let total_items = u32::try_from(self.items.len()).unwrap_or(u32::MAX);
        let start = start.min(total_items);
        let end = start.saturating_add(max_items).min(total_items);
        let window = self.materialized_window();
        let start_index = usize::try_from(start).unwrap_or(usize::MAX);
        let end_index = usize::try_from(end).unwrap_or(usize::MAX);
        let items = self.items[start_index..end_index]
            .iter()
            .zip(&self.starts_milli[start_index..end_index])
            .enumerate()
            .map(|(page_index, (item, start_milli))| {
                let index = start.saturating_add(u32::try_from(page_index).unwrap_or(u32::MAX));
                ViewVirtualRange {
                    index,
                    key: item.key,
                    start_milli: *start_milli,
                    extent_milli: item.extent_milli,
                    materialized: window.contains(index),
                }
            })
            .collect();
        ViewVirtualRangePage {
            total_items,
            start,
            end,
            items,
        }
    }

    /// Publishes one complete table only when an observation/capture asks for it.
    pub fn range_table(&self) -> ViewVirtualRangeTable {
        let page = self.range_page(0, u32::MAX);
        ViewVirtualRangeTable {
            mount: self.mount,
            scroll_target: self.scroll_target.clone(),
            axis: self.axis,
            viewport_extent_milli: self.viewport_extent_milli,
            offset_milli: self.offset_milli,
            total_extent_milli: self.total_extent_milli,
            materialized: self.materialized_window(),
            items: page.items,
        }
    }

    pub fn anchor(&self) -> Option<ViewVirtualAnchor> {
        if self.items.is_empty() {
            return None;
        }
        let index = self
            .starts_milli
            .partition_point(|start| *start <= self.offset_milli)
            .saturating_sub(1)
            .min(self.items.len().saturating_sub(1));
        let item = self.items.get(index)?;
        let start = *self.starts_milli.get(index)?;
        let within = self
            .offset_milli
            .saturating_sub(start)
            .min(u64::from(item.extent_milli));
        Some(ViewVirtualAnchor {
            key: item.key,
            offset_within_item_milli: u32::try_from(within).unwrap_or(u32::MAX),
        })
    }

    pub fn snapshot(&self) -> ViewVirtualListSnapshot {
        ViewVirtualListSnapshot {
            mount: self.mount,
            scroll_target: self.scroll_target.clone(),
            axis: self.axis,
            viewport_extent_milli: self.viewport_extent_milli,
            items: self.items.clone(),
            absolute_offset_milli: self.offset_milli,
            anchor: self.anchor(),
        }
    }

    /// Replaces this list from an exact snapshot without partial mutation.
    pub fn restore(
        &mut self,
        snapshot: &ViewVirtualListSnapshot,
    ) -> Result<(), ViewVirtualizationError> {
        if snapshot.mount != self.mount {
            return Err(ViewVirtualizationError::SnapshotMountMismatch {
                saved: snapshot.mount,
                mounted: self.mount,
            });
        }
        if snapshot.axis != self.axis {
            return Err(ViewVirtualizationError::SnapshotAxisMismatch {
                mount: self.mount,
                saved: snapshot.axis,
                mounted: self.axis,
            });
        }
        if snapshot.scroll_target != self.scroll_target {
            return Err(ViewVirtualizationError::SnapshotScrollTargetMismatch {
                mount: self.mount,
                saved: snapshot.scroll_target.clone(),
                mounted: self.scroll_target.clone(),
            });
        }
        let replacement = Self::from_snapshot(snapshot)?;
        *self = replacement;
        Ok(())
    }

    fn from_snapshot(snapshot: &ViewVirtualListSnapshot) -> Result<Self, ViewVirtualizationError> {
        let mut list = Self::new(
            snapshot.mount,
            snapshot.scroll_target.clone(),
            snapshot.axis,
            snapshot.viewport_extent_milli,
            snapshot.items.clone(),
        )?;
        let maximum = list.max_offset_milli();
        if snapshot.absolute_offset_milli > maximum {
            return Err(ViewVirtualizationError::SnapshotOffsetOutOfRange {
                mount: snapshot.mount,
                saved: snapshot.absolute_offset_milli,
                maximum,
            });
        }
        list.offset_milli = snapshot.absolute_offset_milli;
        if list.anchor() != snapshot.anchor {
            return Err(ViewVirtualizationError::SnapshotAnchorMismatch {
                mount: snapshot.mount,
            });
        }
        Ok(list)
    }

    fn offset_for_anchor(&self, anchor: Option<ViewVirtualAnchor>) -> Option<u64> {
        let anchor = anchor?;
        let index = usize::try_from(*self.indices.get(&anchor.key)?).ok()?;
        let item = self.items.get(index)?;
        let within = anchor.offset_within_item_milli.min(item.extent_milli);
        self.starts_milli.get(index)?.checked_add(u64::from(within))
    }
}

impl ViewVirtualizationRuntime {
    /// Allocates and mounts one occurrence. IDs are never reused after unmount.
    pub fn mount(
        &mut self,
        scroll_target: ViewVirtualScrollTarget,
        axis: ViewVirtualAxis,
        viewport_extent_milli: u32,
        items: Vec<ViewVirtualItem>,
    ) -> Result<ViewMountId, ViewVirtualizationError> {
        let candidate = ViewMountId::from_allocated(self.mount_allocator.next());
        let list =
            ViewVirtualList::new(candidate, scroll_target, axis, viewport_extent_milli, items)?;
        let mount = self
            .mount_allocator
            .allocate()
            .map_err(|_| ViewVirtualizationError::MountIdExhausted)?;
        debug_assert_eq!(mount, candidate);
        match self.mounts.entry(mount) {
            Entry::Vacant(entry) => {
                entry.insert(list);
            }
            Entry::Occupied(_) => {
                return Err(ViewVirtualizationError::MountIdCollision { mount });
            }
        }
        Ok(mount)
    }

    pub fn unmount(&mut self, mount: ViewMountId) -> Option<ViewVirtualList> {
        self.mounts.remove(&mount)
    }

    pub fn get(&self, mount: ViewMountId) -> Option<&ViewVirtualList> {
        self.mounts.get(&mount)
    }

    pub fn get_mut(&mut self, mount: ViewMountId) -> Option<&mut ViewVirtualList> {
        self.mounts.get_mut(&mount)
    }

    pub fn mounts(&self) -> impl ExactSizeIterator<Item = &ViewVirtualList> {
        self.mounts.values()
    }

    pub fn snapshot(&self) -> ViewVirtualizationSnapshot {
        ViewVirtualizationSnapshot {
            next_mount_id: self.mount_allocator.next(),
            mounts: self
                .mounts
                .values()
                .map(ViewVirtualList::snapshot)
                .collect(),
        }
    }

    /// Expands full tables only for explicit observation/capture requests.
    pub fn range_tables(&self) -> Vec<ViewVirtualRangeTable> {
        self.mounts
            .values()
            .map(ViewVirtualList::range_table)
            .collect()
    }

    /// Atomically replaces mounts and the allocator from an exact snapshot.
    pub fn restore(
        &mut self,
        snapshot: &ViewVirtualizationSnapshot,
    ) -> Result<(), ViewVirtualizationError> {
        let mut mounts = BTreeMap::new();
        for saved in &snapshot.mounts {
            if mounts.contains_key(&saved.mount) {
                return Err(ViewVirtualizationError::DuplicateSnapshotMount { mount: saved.mount });
            }
            mounts.insert(saved.mount, ViewVirtualList::from_snapshot(saved)?);
        }
        let greatest = mounts.keys().next_back().copied();
        self.mount_allocator
            .restore_cursor(snapshot.next_mount_id, greatest)
            .map_err(
                |_| ViewVirtualizationError::SnapshotMountAllocatorNotFresh {
                    next_mount_id: snapshot.next_mount_id,
                    greatest_mount_id: greatest.map_or(0, ViewMountId::get),
                },
            )?;
        self.mounts = mounts;
        Ok(())
    }

    pub fn from_snapshot(
        snapshot: &ViewVirtualizationSnapshot,
    ) -> Result<Self, ViewVirtualizationError> {
        let mut runtime = Self::default();
        runtime.restore(snapshot)?;
        Ok(runtime)
    }
}

fn index_items(items: &[ViewVirtualItem]) -> Result<IndexedItems, ViewVirtualizationError> {
    let _item_count =
        u32::try_from(items.len()).map_err(|_| ViewVirtualizationError::ItemCapacityExceeded)?;
    let mut starts_milli = Vec::with_capacity(items.len());
    let mut indices = BTreeMap::new();
    let mut total = 0_u64;
    for (index, item) in items.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ViewVirtualizationError::ItemCapacityExceeded)?;
        if item.extent_milli == 0 {
            return Err(ViewVirtualizationError::ZeroItemExtent { index });
        }
        if indices.insert(item.key, index).is_some() {
            return Err(ViewVirtualizationError::DuplicateItemKey { key: item.key });
        }
        starts_milli.push(total);
        // With at most u32::MAX items and u32 extents, this sum fits in u64.
        total += u64::from(item.extent_milli);
    }
    Ok(IndexedItems {
        starts_milli,
        indices,
        total_extent_milli: total,
    })
}
