//! Deterministic Arcweft standard data types.
//!
//! These types are Sans I/O data containers for the facade prelude. Runtime or
//! adapter crates can choose faster backend structures internally, but anything
//! visible to replay, save data, diagnostics, or authored order should keep a
//! deterministic iteration contract.

use core::hash::{Hash, Hasher};
use std::collections::{BTreeMap as StdBTreeMap, BTreeSet as StdBTreeSet, VecDeque as StdVecDeque};

/// Arcweft unit value.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Unit;

/// Arcweft bottom type. It has no values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Never {}

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

/// Default Arcweft map: stable authored/insertion order.
pub type Map<K, V> = OrderedMap<K, V>;

/// Default Arcweft set: stable authored/insertion order.
pub type Set<T> = OrderedSet<T>;

/// Deterministic set used for flags, capabilities, and effect summaries.
pub type BitSet<E> = StdBTreeSet<E>;

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

#[cfg(test)]
mod tests {
    use super::{Array, OrderedMap, Vec};

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
}
