/// Inclusive limits for one resource extension-manifest decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceManifestDecodeLimits {
    bytes: usize,
    nesting_depth: usize,
    lexical_nodes: usize,
    string_bytes: usize,
    collection_items: usize,
    object_members: usize,
    semantic_records: usize,
    work_units: u64,
}

/// Aggregate publication limits applied after each document has decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceManifestPublicationLimits {
    semantic_records: usize,
    work_units: u64,
}

impl ResourceManifestDecodeLimits {
    pub const PRODUCTION: Self = Self::new(
        8_388_608, 64, 65_536, 1_048_576, 16_384, 4_096, 16_384, 1_048_576,
    );

    #[allow(
        clippy::too_many_arguments,
        reason = "the contract defines eight independently tested resource limits"
    )]
    pub const fn new(
        bytes: usize,
        nesting_depth: usize,
        lexical_nodes: usize,
        string_bytes: usize,
        collection_items: usize,
        object_members: usize,
        semantic_records: usize,
        work_units: u64,
    ) -> Self {
        Self {
            bytes,
            nesting_depth,
            lexical_nodes,
            string_bytes,
            collection_items,
            object_members,
            semantic_records,
            work_units,
        }
    }

    pub const fn bytes(self) -> usize {
        self.bytes
    }
    pub const fn nesting_depth(self) -> usize {
        self.nesting_depth
    }
    pub const fn lexical_nodes(self) -> usize {
        self.lexical_nodes
    }
    pub const fn string_bytes(self) -> usize {
        self.string_bytes
    }
    pub const fn collection_items(self) -> usize {
        self.collection_items
    }
    pub const fn object_members(self) -> usize {
        self.object_members
    }
    pub const fn semantic_records(self) -> usize {
        self.semantic_records
    }
    pub const fn work_units(self) -> u64 {
        self.work_units
    }
}

impl Default for ResourceManifestDecodeLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

impl ResourceManifestPublicationLimits {
    pub const PRODUCTION: Self = Self::new(16_384, 1_048_576);

    pub const fn new(semantic_records: usize, work_units: u64) -> Self {
        Self {
            semantic_records,
            work_units,
        }
    }

    pub const fn semantic_records(self) -> usize {
        self.semantic_records
    }
    pub const fn work_units(self) -> u64 {
        self.work_units
    }
}

impl Default for ResourceManifestPublicationLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}
