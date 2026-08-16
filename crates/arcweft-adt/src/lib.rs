//! Deterministic Arcweft standard data types.
//!
//! These types are Sans I/O data containers for the facade prelude. Runtime or
//! adapter crates can choose faster backend structures internally, but anything
//! visible to replay, save data, diagnostics, or authored order should keep a
//! deterministic iteration contract.

use core::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    marker::PhantomData,
};
use std::collections::{BTreeMap as StdBTreeMap, BTreeSet as StdBTreeSet, VecDeque as StdVecDeque};

/// Arcweft unit value.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Unit;

/// Arcweft bottom type. It has no values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Never {}

/// Stable index used by arena-like data structures.
#[derive(Debug)]
pub struct ArenaId<T> {
    index: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> ArenaId<T> {
    /// Creates an arena ID from a zero-based index.
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            marker: PhantomData,
        }
    }

    /// Zero-based index.
    pub const fn index(self) -> u32 {
        self.index
    }
}

impl<T> Clone for ArenaId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ArenaId<T> {}

impl<T> PartialEq for ArenaId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for ArenaId<T> {}

impl<T> PartialOrd for ArenaId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for ArenaId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T> Hash for ArenaId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

/// Append-only arena for compiler/runtime data that is stable by insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arena<T> {
    items: std::vec::Vec<T>,
}

impl<T> Arena<T> {
    /// Creates an empty arena.
    pub const fn new() -> Self {
        Self {
            items: std::vec::Vec::new(),
        }
    }

    /// Inserts a value and returns its stable arena ID.
    ///
    /// # Panics
    ///
    /// Panics if more than `u32::MAX` values are inserted. Arcweft IDs are kept
    /// compact for manifests and replay tables.
    pub fn insert(&mut self, value: T) -> ArenaId<T> {
        let id = ArenaId::new(u32::try_from(self.items.len()).expect("arena index fits in u32"));
        self.items.push(value);
        id
    }

    /// Gets a value by arena ID.
    pub fn get(&self, id: ArenaId<T>) -> Option<&T> {
        self.items.get(id.index as usize)
    }

    /// Iterates values in insertion order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T> IntoIterator for &'a Arena<T> {
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Arena whose IDs are intended to survive save/replay/debug manifests.
pub type StableArena<T> = Arena<T>;

/// Frame-local arena contract.
pub type FrameArena<T> = Arena<T>;

/// Bump-style arena contract. The Rust MVP keeps the same append-only data
/// shape; allocator strategy is not part of the serialized contract.
pub type BumpArena<T> = Arena<T>;

/// Stable AST node ID for compiler/tooling manifests.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AstNodeId(pub u32);

/// Stable HIR node ID for compiler/tooling manifests.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirId(pub u32);

/// Generational ID used to detect stale slot handles.
#[derive(Debug)]
pub struct GenerationalId<T> {
    index: u32,
    generation: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> GenerationalId<T> {
    /// Creates a generational ID.
    pub const fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            marker: PhantomData,
        }
    }

    /// Slot index.
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Slot generation.
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> Clone for GenerationalId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for GenerationalId<T> {}

impl<T> PartialEq for GenerationalId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for GenerationalId<T> {}

impl<T> PartialOrd for GenerationalId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for GenerationalId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.index, self.generation).cmp(&(other.index, other.generation))
    }
}

impl<T> Hash for GenerationalId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SlotEntry<T> {
    generation: u32,
    value: Option<T>,
}

/// Generational slot storage for runtime handles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotMap<T> {
    slots: std::vec::Vec<SlotEntry<T>>,
    free: std::vec::Vec<u32>,
}

impl<T> SlotMap<T> {
    /// Creates an empty slot map.
    pub const fn new() -> Self {
        Self {
            slots: std::vec::Vec::new(),
            free: std::vec::Vec::new(),
        }
    }

    /// Inserts a value and returns a generational ID.
    ///
    /// # Panics
    ///
    /// Panics if more than `u32::MAX` slots are allocated over one map's live
    /// storage. Arcweft generational IDs are intentionally compact.
    pub fn insert(&mut self, value: T) -> GenerationalId<T> {
        if let Some(index) = self.free.pop() {
            let entry = &mut self.slots[index as usize];
            entry.value = Some(value);
            return GenerationalId::new(index, entry.generation);
        }
        let index = u32::try_from(self.slots.len()).expect("slot index fits in u32");
        self.slots.push(SlotEntry {
            generation: 0,
            value: Some(value),
        });
        GenerationalId::new(index, 0)
    }

    /// Gets a value if the ID is still live.
    pub fn get(&self, id: GenerationalId<T>) -> Option<&T> {
        self.slots
            .get(id.index as usize)
            .filter(|entry| entry.generation == id.generation)
            .and_then(|entry| entry.value.as_ref())
    }

    /// Removes a value and invalidates existing IDs for that slot.
    pub fn remove(&mut self, id: GenerationalId<T>) -> Option<T> {
        let entry = self.slots.get_mut(id.index as usize)?;
        if entry.generation != id.generation {
            return None;
        }
        let value = entry.value.take()?;
        entry.generation = entry.generation.wrapping_add(1);
        self.free.push(id.index);
        Some(value)
    }
}

impl<T> Default for SlotMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Sparse deterministic set keyed by typed generational IDs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SparseSet<T> {
    ids: BTreeSet<GenerationalId<T>>,
}

impl<T> SparseSet<T> {
    /// Inserts an ID.
    pub fn insert(&mut self, id: GenerationalId<T>) -> bool {
        self.ids.insert(id)
    }

    /// Returns true if the ID is present.
    pub fn contains(&self, id: &GenerationalId<T>) -> bool {
        self.ids.contains(id)
    }

    /// Iterates IDs in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = &GenerationalId<T>> {
        self.ids.iter()
    }
}

/// Small typed entity store backed by a generational slot map.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EntityStore<T> {
    slots: SlotMap<T>,
}

impl<T> EntityStore<T> {
    /// Inserts an entity value.
    pub fn insert(&mut self, value: T) -> GenerationalId<T> {
        self.slots.insert(value)
    }

    /// Gets an entity by ID.
    pub fn get(&self, id: GenerationalId<T>) -> Option<&T> {
        self.slots.get(id)
    }

    /// Removes an entity by ID.
    pub fn remove(&mut self, id: GenerationalId<T>) -> Option<T> {
        self.slots.remove(id)
    }
}

/// Dynamic ordered sequence. This is Arcweft's default list-like collection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Vec<T>(std::vec::Vec<T>);

impl<T> Vec<T> {
    /// Creates an empty vector.
    pub const fn new() -> Self {
        Self(std::vec::Vec::new())
    }

    /// Creates an empty vector with reserved capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(std::vec::Vec::with_capacity(capacity))
    }

    /// Adds one item to the end.
    pub fn push(&mut self, item: T) {
        self.0.push(item);
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no items.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Shrinks backing storage as much as the host allocator allows.
    pub fn shrink(&mut self) {
        self.0.shrink_to_fit();
    }

    /// Shrinks backing storage while keeping at least `min_capacity`.
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }

    /// Borrows the vector as a slice.
    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    /// Iterates in authored/insertion order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    /// Converts into the underlying Rust `Vec`.
    pub fn into_std(self) -> std::vec::Vec<T> {
        self.0
    }
}

impl<T> From<std::vec::Vec<T>> for Vec<T> {
    fn from(items: std::vec::Vec<T>) -> Self {
        Self(items)
    }
}

impl<T, const N: usize> From<[T; N]> for Vec<T> {
    fn from(items: [T; N]) -> Self {
        Self(std::vec::Vec::from(items))
    }
}

impl<T> FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(std::vec::Vec::from_iter(iter))
    }
}

impl<T> IntoIterator for Vec<T> {
    type IntoIter = std::vec::IntoIter<T>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Vec<T> {
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Fixed-length sequence. Arcweft `[a, b]` can lower to this when expected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Array<T, const N: usize>([T; N]);

impl<T, const N: usize> Array<T, N> {
    /// Creates a fixed-length array from exact storage.
    pub const fn new(items: [T; N]) -> Self {
        Self(items)
    }

    /// Number of items, known at compile time.
    pub const fn len(&self) -> usize {
        N
    }

    /// Returns true when `N == 0`.
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    /// Borrows the array as a slice.
    pub const fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Converts into the underlying Rust array.
    pub fn into_inner(self) -> [T; N] {
        self.0
    }
}

impl<T, const N: usize> From<[T; N]> for Array<T, N> {
    fn from(items: [T; N]) -> Self {
        Self(items)
    }
}

impl<T, const N: usize> IntoIterator for Array<T, N> {
    type IntoIter = std::array::IntoIter<T, N>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Double-ended queue for scheduler and event queues.
pub type VecDeque<T> = StdVecDeque<T>;

/// Canonically sorted map for replay-stable serialization.
pub type BTreeMap<K, V> = StdBTreeMap<K, V>;

/// Canonically sorted set for replay-stable serialization.
pub type BTreeSet<T> = StdBTreeSet<T>;

/// Sorted map name used by Arcweft surface docs.
pub type SortedMap<K, V> = StdBTreeMap<K, V>;

/// Sorted set name used by Arcweft surface docs.
pub type SortedSet<T> = StdBTreeSet<T>;

/// Insertion-ordered deterministic map.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OrderedMap<K, V> {
    entries: std::vec::Vec<(K, V)>,
}

impl<K: Eq, V> OrderedMap<K, V> {
    /// Creates an empty insertion-ordered map.
    pub const fn new() -> Self {
        Self {
            entries: std::vec::Vec::new(),
        }
    }

    /// Inserts or replaces a value while preserving first insertion order.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some((_, existing)) = self
            .entries
            .iter_mut()
            .find(|(existing, _)| *existing == key)
        {
            return Some(core::mem::replace(existing, value));
        }
        self.entries.push((key, value));
        None
    }

    /// Gets a value by key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .iter()
            .find_map(|(existing, value)| (existing == key).then_some(value))
    }

    /// Iterates in first insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the map has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Insertion-ordered deterministic set.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OrderedSet<T> {
    entries: std::vec::Vec<T>,
}

impl<T: Eq> OrderedSet<T> {
    /// Creates an empty insertion-ordered set.
    pub const fn new() -> Self {
        Self {
            entries: std::vec::Vec::new(),
        }
    }

    /// Inserts a value if it does not exist yet.
    pub fn insert(&mut self, value: T) -> bool {
        if self.entries.contains(&value) {
            return false;
        }
        self.entries.push(value);
        true
    }

    /// Iterates in first insertion order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.entries.iter()
    }
}

impl<'a, T: Eq> IntoIterator for &'a OrderedSet<T> {
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Deterministic set used for flags, capabilities, and effect summaries.
pub type BitSet<E> = StdBTreeSet<E>;

/// Small deterministic list. The current implementation is Vec-backed; the
/// type name preserves intent without committing the data format to inline
/// storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmallList<T, const N: usize> {
    items: std::vec::Vec<T>,
}

impl<T, const N: usize> SmallList<T, N> {
    /// Creates an empty list with capacity for the common inline target size.
    pub fn new() -> Self {
        Self {
            items: std::vec::Vec::with_capacity(N),
        }
    }

    /// Adds one item to the end.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// Borrows the stored items.
    pub fn as_slice(&self) -> &[T] {
        self.items.as_slice()
    }
}

impl<T, const N: usize> Default for SmallList<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Dotted path into deterministic state.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatePath(std::vec::Vec<String>);

impl StatePath {
    /// Creates a state path from pre-split components.
    pub fn new(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(parts.into_iter().map(Into::into).collect())
    }

    /// Parses a dot-separated state path.
    pub fn dotted(path: &str) -> Self {
        Self::new(path.split('.').filter(|part| !part.is_empty()))
    }

    /// Path components.
    pub fn parts(&self) -> &[String] {
        self.0.as_slice()
    }

    /// Dot-separated representation.
    pub fn as_dotted(&self) -> String {
        self.0.join(".")
    }
}

/// Stable hash bytes for snapshots, manifests, and traces.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StableHash([u8; 32]);

impl StableHash {
    /// Creates a stable hash from exactly 32 bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the stored hash bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Hash for StableHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Typed state patch payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Patch<T> {
    value: T,
}

impl<T> Patch<T> {
    /// Creates a patch from a value.
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Borrows the patch value.
    pub const fn value(&self) -> &T {
        &self.value
    }
}

/// Ordered group of state patches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchSet<T> {
    patches: Vec<Patch<T>>,
}

impl<T> PatchSet<T> {
    /// Appends one patch.
    pub fn push(&mut self, patch: Patch<T>) {
        self.patches.push(patch);
    }

    /// Iterates patches in deterministic append order.
    pub fn iter(&self) -> std::slice::Iter<'_, Patch<T>> {
        self.patches.iter()
    }
}

impl<T> Default for PatchSet<T> {
    fn default() -> Self {
        Self {
            patches: Vec::new(),
        }
    }
}

impl<'a, T> IntoIterator for &'a PatchSet<T> {
    type IntoIter = std::slice::Iter<'a, Patch<T>>;
    type Item = &'a Patch<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Before/after value delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diff<T> {
    before: Option<T>,
    after: Option<T>,
}

impl<T> Diff<T> {
    /// Creates a diff from optional before/after values.
    pub const fn new(before: Option<T>, after: Option<T>) -> Self {
        Self { before, after }
    }

    /// Value before the change.
    pub const fn before(&self) -> Option<&T> {
        self.before.as_ref()
    }

    /// Value after the change.
    pub const fn after(&self) -> Option<&T> {
        self.after.as_ref()
    }
}

/// Value tagged with a deterministic version counter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Versioned<T> {
    version: u64,
    value: T,
}

impl<T> Versioned<T> {
    /// Creates a versioned value.
    pub const fn new(version: u64, value: T) -> Self {
        Self { version, value }
    }

    /// Version counter.
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Stored value.
    pub const fn value(&self) -> &T {
        &self.value
    }
}

/// Append-only deterministic event log.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventLog<E> {
    events: Vec<E>,
}

impl<E> EventLog<E> {
    /// Appends one event.
    pub fn push(&mut self, event: E) {
        self.events.push(event);
    }

    /// Iterates events in append order.
    pub fn iter(&self) -> std::slice::Iter<'_, E> {
        self.events.iter()
    }
}

impl<'a, E> IntoIterator for &'a EventLog<E> {
    type IntoIter = std::slice::Iter<'a, E>;
    type Item = &'a E;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Trace log is an event log with trace-specific naming.
pub type TraceLog<E> = EventLog<E>;

/// FIFO task queue for Sans I/O runtime plans.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskQueue<T> {
    items: VecDeque<T>,
}

impl<T> TaskQueue<T> {
    /// Pushes a task to the back.
    pub fn push_back(&mut self, task: T) {
        self.items.push_back(task);
    }

    /// Pops the next task from the front.
    pub fn pop_front(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    /// Number of queued tasks.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if no tasks are queued.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// FIFO event queue for deterministic adapter boundaries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventQueue<E> {
    items: VecDeque<E>,
}

impl<E> EventQueue<E> {
    /// Pushes an event to the back.
    pub fn push_back(&mut self, event: E) {
        self.items.push_back(event);
    }

    /// Pops the next event from the front.
    pub fn pop_front(&mut self) -> Option<E> {
        self.items.pop_front()
    }
}

/// Cached Need state without depending on the runtime Need crate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NeedCacheState<T, E, P = ()> {
    #[default]
    Empty,
    Pending(P),
    Ready(T),
    Err(E),
    Cancelled,
}

/// Ordered stream transform descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream<T, E> {
    key: String,
    marker: PhantomData<fn() -> (T, E)>,
}

impl<T, E> Stream<T, E> {
    /// Creates a stream descriptor from a stable key.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            marker: PhantomData,
        }
    }

    /// Stable stream key.
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Immutable snapshot wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot<T> {
    value: T,
    hash: Option<StableHash>,
}

impl<T> Snapshot<T> {
    /// Creates a snapshot without an external hash.
    pub fn new(value: T) -> Self {
        Self { value, hash: None }
    }

    /// Attaches a stable hash to a snapshot.
    pub fn with_hash(value: T, hash: StableHash) -> Self {
        Self {
            value,
            hash: Some(hash),
        }
    }

    /// Snapshot value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Optional stable hash.
    pub const fn hash(&self) -> Option<StableHash> {
        self.hash
    }
}

/// Stable tree node ID.
#[derive(Debug)]
pub struct TreeNodeId<T> {
    index: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> TreeNodeId<T> {
    /// Creates a tree node ID from a zero-based index.
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            marker: PhantomData,
        }
    }

    /// Zero-based node index.
    pub const fn index(self) -> u32 {
        self.index
    }
}

impl<T> Clone for TreeNodeId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TreeNodeId<T> {}

impl<T> PartialEq for TreeNodeId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for TreeNodeId<T> {}

impl<T> PartialOrd for TreeNodeId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for TreeNodeId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T> Hash for TreeNodeId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

/// One node in a deterministic tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeNode<T> {
    value: T,
    parent: Option<TreeNodeId<T>>,
    children: std::vec::Vec<TreeNodeId<T>>,
}

impl<T> TreeNode<T> {
    /// Stored node value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Parent node, if any.
    pub const fn parent(&self) -> Option<TreeNodeId<T>> {
        self.parent
    }

    /// Child nodes in insertion order.
    pub fn children(&self) -> &[TreeNodeId<T>] {
        self.children.as_slice()
    }
}

/// Deterministic parent/child tree used by layer, scene, and View structures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tree<T> {
    nodes: Arena<TreeNode<T>>,
    roots: std::vec::Vec<TreeNodeId<T>>,
}

impl<T> Tree<T> {
    /// Creates an empty tree.
    pub const fn new() -> Self {
        Self {
            nodes: Arena::new(),
            roots: std::vec::Vec::new(),
        }
    }

    /// Inserts a root node.
    pub fn push_root(&mut self, value: T) -> TreeNodeId<T> {
        let raw = self.nodes.insert(TreeNode {
            value,
            parent: None,
            children: std::vec::Vec::new(),
        });
        let id = TreeNodeId::new(raw.index());
        self.roots.push(id);
        id
    }

    /// Inserts a child below `parent`.
    pub fn push_child(&mut self, parent: TreeNodeId<T>, value: T) -> Option<TreeNodeId<T>> {
        self.nodes.get(ArenaId::new(parent.index()))?;
        let raw = self.nodes.insert(TreeNode {
            value,
            parent: Some(parent),
            children: std::vec::Vec::new(),
        });
        let id = TreeNodeId::new(raw.index());
        self.nodes.items[parent.index() as usize].children.push(id);
        Some(id)
    }

    /// Gets a node by ID.
    pub fn get(&self, id: TreeNodeId<T>) -> Option<&TreeNode<T>> {
        self.nodes.get(ArenaId::new(id.index()))
    }

    /// Root nodes in insertion order.
    pub fn roots(&self) -> &[TreeNodeId<T>] {
        self.roots.as_slice()
    }
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Layer tree shares the same deterministic tree contract.
pub type LayerTree<T> = Tree<T>;

/// Scene graph shares the same deterministic tree contract.
pub type SceneGraph<T> = Tree<T>;

/// View tree shares the same deterministic tree contract.
pub type ViewTree<T> = Tree<T>;

/// Stable graph node ID.
#[derive(Debug)]
pub struct GraphNodeId<T> {
    index: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> GraphNodeId<T> {
    /// Creates a graph node ID from a zero-based index.
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            marker: PhantomData,
        }
    }

    /// Zero-based node index.
    pub const fn index(self) -> u32 {
        self.index
    }
}

impl<T> Clone for GraphNodeId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for GraphNodeId<T> {}

impl<T> PartialEq for GraphNodeId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for GraphNodeId<T> {}

impl<T> PartialOrd for GraphNodeId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for GraphNodeId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T> Hash for GraphNodeId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

/// One deterministic directed graph edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphEdge<E> {
    pub from: u32,
    pub to: u32,
    pub value: E,
}

/// Directed graph with stable node order and explicit edge order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableGraph<N, E> {
    nodes: Arena<N>,
    edges: std::vec::Vec<GraphEdge<E>>,
}

impl<N, E> StableGraph<N, E> {
    /// Creates an empty graph.
    pub const fn new() -> Self {
        Self {
            nodes: Arena::new(),
            edges: std::vec::Vec::new(),
        }
    }

    /// Inserts a graph node.
    pub fn add_node(&mut self, node: N) -> GraphNodeId<N> {
        GraphNodeId::new(self.nodes.insert(node).index())
    }

    /// Adds an edge if both endpoints exist.
    pub fn add_edge(&mut self, from: GraphNodeId<N>, to: GraphNodeId<N>, value: E) -> bool {
        if self.nodes.get(ArenaId::new(from.index())).is_none()
            || self.nodes.get(ArenaId::new(to.index())).is_none()
        {
            return false;
        }
        self.edges.push(GraphEdge {
            from: from.index(),
            to: to.index(),
            value,
        });
        true
    }

    /// Gets a graph node.
    pub fn node(&self, id: GraphNodeId<N>) -> Option<&N> {
        self.nodes.get(ArenaId::new(id.index()))
    }

    /// Edges in deterministic insertion order.
    pub fn edges(&self) -> &[GraphEdge<E>] {
        self.edges.as_slice()
    }
}

impl<N, E> Default for StableGraph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}

/// Dependency graph with unit edge payloads.
pub type DependencyGraph<N> = StableGraph<N, Unit>;

/// Route graph with string node and edge labels for manifests/tooling.
pub type RouteGraph = StableGraph<String, String>;

/// Bounded FIFO ring buffer for source and data-plane queues.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RingBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> RingBuffer<T> {
    /// Creates an empty ring buffer with a maximum item count.
    pub const fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            items: VecDeque::new(),
        }
    }

    /// Pushes an item and drops the oldest item when full.
    pub fn push_back(&mut self, value: T) -> Option<T> {
        let evicted = (self.items.len() == self.capacity)
            .then(|| self.items.pop_front())
            .flatten();
        if self.capacity > 0 {
            self.items.push_back(value);
        }
        evicted
    }

    /// Pops the oldest item.
    pub fn pop_front(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    /// Number of buffered items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true when no items are buffered.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Last-value signal cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signal<T> {
    value: T,
}

impl<T> Signal<T> {
    /// Creates a signal with an initial value.
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Reads the current value.
    pub const fn get(&self) -> &T {
        &self.value
    }

    /// Replaces the signal value and returns the previous value.
    pub fn set(&mut self, value: T) -> T {
        core::mem::replace(&mut self.value, value)
    }
}

/// Deterministic map of named signals.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SignalBus<T> {
    signals: BTreeMap<String, Signal<T>>,
}

impl<T> SignalBus<T> {
    /// Inserts or replaces a named signal.
    pub fn set(&mut self, name: impl Into<String>, value: T) -> Option<Signal<T>> {
        self.signals.insert(name.into(), Signal::new(value))
    }

    /// Gets a signal by name.
    pub fn get(&self, name: &str) -> Option<&Signal<T>> {
        self.signals.get(name)
    }
}

/// Localized map keyed by BCP-47-like locale strings.
pub type LocaleMap<T> = BTreeMap<String, T>;

/// Localized value with deterministic locale ordering.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Localized<T> {
    entries: LocaleMap<T>,
}

impl<T> Localized<T> {
    /// Inserts a localized value.
    pub fn insert(&mut self, locale: impl Into<String>, value: T) -> Option<T> {
        self.entries.insert(locale.into(), value)
    }

    /// Gets a localized value.
    pub fn get(&self, locale: &str) -> Option<&T> {
        self.entries.get(locale)
    }
}

/// Inline rich-text tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineTag {
    name: String,
    attrs: OrderedMap<String, String>,
}

impl InlineTag {
    /// Creates an inline tag with no attributes.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: OrderedMap::new(),
        }
    }

    /// Tag name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Adds or replaces an attribute.
    pub fn insert_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attrs.insert(name.into(), value.into());
    }
}

/// Ruby annotation attached to a base text span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RubyText {
    base: String,
    ruby: String,
}

impl RubyText {
    /// Creates ruby text.
    pub fn new(base: impl Into<String>, ruby: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            ruby: ruby.into(),
        }
    }
}

/// One rich-text run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextRun {
    Text(String),
    Tag(InlineTag),
    Ruby(RubyText),
}

/// Ordered rich-text content.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RichText {
    runs: std::vec::Vec<TextRun>,
}

impl RichText {
    /// Appends one run.
    pub fn push(&mut self, run: TextRun) {
        self.runs.push(run);
    }

    /// Runs in authored order.
    pub fn runs(&self) -> &[TextRun] {
        self.runs.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Arena, Array, DependencyGraph, EntityStore, NeedCacheState, OrderedMap, Patch, PatchSet,
        RichText, RingBuffer, SignalBus, StatePath, TaskQueue, TextRun, Tree, Vec,
    };

    #[test]
    fn vec_preserves_order_and_shrinks() {
        let mut values = Vec::with_capacity(8);
        values.push(1);
        values.push(2);
        values.shrink();
        assert_eq!(values.as_slice(), &[1, 2]);
    }

    #[test]
    fn array_has_fixed_length() {
        let values = Array::new([1, 2, 3]);
        assert_eq!(values.len(), 3);
        assert_eq!(values.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn ordered_map_replaces_without_reordering() {
        let mut map = OrderedMap::new();
        assert_eq!(map.insert("b", 1), None);
        assert_eq!(map.insert("a", 2), None);
        assert_eq!(map.insert("b", 3), Some(1));
        let keys = map
            .iter()
            .map(|(key, _)| *key)
            .collect::<std::vec::Vec<_>>();
        assert_eq!(keys, ["b", "a"]);
        assert_eq!(map.get(&"b"), Some(&3));
    }

    #[test]
    fn state_path_preserves_components() {
        let path = StatePath::dotted("flow.opening.flags.ready");
        assert_eq!(path.parts(), &["flow", "opening", "flags", "ready"]);
        assert_eq!(path.as_dotted(), "flow.opening.flags.ready");
    }

    #[test]
    fn patch_set_and_task_queue_keep_order() {
        let mut patches = PatchSet::default();
        patches.push(Patch::new(1));
        patches.push(Patch::new(2));
        let values = patches
            .iter()
            .map(|patch| *patch.value())
            .collect::<std::vec::Vec<_>>();
        assert_eq!(values, [1, 2]);

        let mut queue = TaskQueue::default();
        queue.push_back("load");
        queue.push_back("show");
        assert_eq!(queue.pop_front(), Some("load"));
        assert_eq!(queue.pop_front(), Some("show"));
    }

    #[test]
    fn need_cache_state_is_pure_data() {
        let state: NeedCacheState<i32, &str, u8> = NeedCacheState::Pending(2);
        assert_eq!(state, NeedCacheState::Pending(2));
    }

    #[test]
    fn arena_tree_and_graph_keep_stable_order() {
        let mut arena = Arena::new();
        let a = arena.insert("a");
        let b = arena.insert("b");
        assert_eq!(a.index(), 0);
        assert_eq!(b.index(), 1);
        assert_eq!(arena.get(a), Some(&"a"));

        let mut tree = Tree::new();
        let root = tree.push_root("root");
        let child = tree.push_child(root, "child").expect("child");
        assert_eq!(tree.roots()[0].index(), 0);
        assert_eq!(tree.get(root).expect("root").children(), &[child]);
        assert_eq!(tree.get(child).expect("child").parent(), Some(root));

        let mut graph = DependencyGraph::new();
        let first = graph.add_node("asset.a");
        let second = graph.add_node("asset.b");
        assert!(graph.add_edge(first, second, super::Unit));
        assert_eq!(graph.edges()[0].from, 0);
        assert_eq!(graph.edges()[0].to, 1);
    }

    #[test]
    fn slot_store_detects_stale_generations() {
        let mut store = EntityStore::default();
        let first = store.insert("first");
        assert_eq!(store.get(first), Some(&"first"));
        assert_eq!(store.remove(first), Some("first"));
        assert_eq!(store.get(first), None);
        let second = store.insert("second");
        assert_ne!(first.generation(), second.generation());
        assert_eq!(store.get(second), Some(&"second"));
    }

    #[test]
    fn ring_buffer_signal_and_rich_text_are_pure_data() {
        let mut ring = RingBuffer::with_capacity(2);
        assert_eq!(ring.push_back(1), None);
        assert_eq!(ring.push_back(2), None);
        assert_eq!(ring.push_back(3), Some(1));
        assert_eq!(ring.pop_front(), Some(2));

        let mut bus = SignalBus::default();
        assert_eq!(bus.set("ready", true), None);
        assert_eq!(bus.get("ready").expect("signal").get(), &true);

        let mut text = RichText::default();
        text.push(TextRun::Text("hello".to_owned()));
        assert_eq!(text.runs(), &[TextRun::Text("hello".to_owned())]);
    }
}
