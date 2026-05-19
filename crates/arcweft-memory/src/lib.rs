//! Sans I/O memory and zero-copy descriptors.
//!
//! This crate deliberately stores data and layout descriptors only. File I/O,
//! mmap setup, shared-memory allocation, and platform handles belong in host
//! adapter crates.

/// Owned byte buffer for non-shared Arcweft data.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bytes(std::vec::Vec<u8>);

impl Bytes {
    /// Creates an empty byte buffer.
    pub const fn new() -> Self {
        Self(std::vec::Vec::new())
    }

    /// Creates a buffer with reserved capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(std::vec::Vec::with_capacity(capacity))
    }

    /// Appends one byte.
    pub fn push(&mut self, byte: u8) {
        self.0.push(byte);
    }

    /// Shrinks backing storage as much as the host allocator allows.
    pub fn shrink(&mut self) {
        self.0.shrink_to_fit();
    }

    /// Byte slice.
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl From<std::vec::Vec<u8>> for Bytes {
    fn from(bytes: std::vec::Vec<u8>) -> Self {
        Self(bytes)
    }
}

/// Immutable named binary payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Blob {
    bytes: Bytes,
}

impl Blob {
    /// Creates a blob from owned bytes.
    pub const fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    /// Blob bytes.
    pub const fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

/// Stable reference to a blob in a bundle or asset table.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlobRef(String);

impl BlobRef {
    /// Creates a blob reference from a stable key.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Reference key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Descriptor for a byte range inside a shared backing store.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SharedSliceDesc {
    offset: u64,
    len: u64,
}

impl SharedSliceDesc {
    /// Creates a shared-slice descriptor.
    pub const fn new(offset: u64, len: u64) -> Self {
        Self { offset, len }
    }

    /// Start offset in bytes.
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Length in bytes.
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Returns true for an empty slice.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Lease descriptor for an externally managed memory region.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryLease {
    key: String,
}

impl MemoryLease {
    /// Creates a memory lease key. Allocation is handled by an adapter.
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }

    /// Lease key.
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Marker for POD slices in shared memory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PodSlice<T> {
    desc: SharedSliceDesc,
    marker: core::marker::PhantomData<fn() -> T>,
}

impl<T> PodSlice<T> {
    /// Creates a typed POD slice descriptor.
    pub const fn new(desc: SharedSliceDesc) -> Self {
        Self {
            desc,
            marker: core::marker::PhantomData,
        }
    }

    /// Untyped layout descriptor.
    pub const fn desc(&self) -> SharedSliceDesc {
        self.desc
    }
}

/// Typed shared slice descriptor for non-POD logical payloads.
///
/// The descriptor carries only layout and type information. Mapping,
/// validation, and host lifetime management belong to adapter crates.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SharedSlice<T> {
    desc: SharedSliceDesc,
    marker: core::marker::PhantomData<fn() -> T>,
}

impl<T> SharedSlice<T> {
    /// Creates a typed shared-slice descriptor.
    pub const fn new(desc: SharedSliceDesc) -> Self {
        Self {
            desc,
            marker: core::marker::PhantomData,
        }
    }

    /// Untyped layout descriptor.
    pub const fn desc(&self) -> SharedSliceDesc {
        self.desc
    }
}

#[cfg(test)]
mod tests {
    use super::{Bytes, PodSlice, SharedSlice, SharedSliceDesc};

    #[test]
    fn bytes_shrink_keeps_data() {
        let mut bytes = Bytes::with_capacity(8);
        bytes.push(1);
        bytes.push(2);
        bytes.shrink();
        assert_eq!(bytes.as_slice(), &[1, 2]);
    }

    #[test]
    fn pod_slice_is_descriptor_only() {
        let desc = SharedSliceDesc::new(16, 32);
        let slice = PodSlice::<u32>::new(desc);
        assert_eq!(slice.desc().offset(), 16);
        assert_eq!(slice.desc().len(), 32);
    }

    #[test]
    fn shared_slice_is_descriptor_only() {
        let desc = SharedSliceDesc::new(64, 128);
        let slice = SharedSlice::<String>::new(desc);
        assert_eq!(slice.desc().offset(), 64);
        assert_eq!(slice.desc().len(), 128);
    }
}
