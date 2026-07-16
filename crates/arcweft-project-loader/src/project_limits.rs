/// Inclusive caller-supplied limits for one project load transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectLoadLimits {
    documents: u64,
    source_bytes: u64,
}

impl ProjectLoadLimits {
    pub const fn new(documents: u64, source_bytes: u64) -> Self {
        Self {
            documents,
            source_bytes,
        }
    }

    pub const fn documents(self) -> u64 {
        self.documents
    }

    pub const fn source_bytes(self) -> u64 {
        self.source_bytes
    }
}
