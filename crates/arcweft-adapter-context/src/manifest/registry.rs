//! Typed adapter-manifest collection and checked lookup.

use thiserror::Error;

use super::{AdapterId, AdapterManifest};

/// Collection used to resolve launch-profile adapter ids.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterRegistry {
    manifests: Vec<AdapterManifest>,
}

/// Failure to insert an adapter manifest into a checked registry.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AdapterRegistryError {
    /// Two manifests declared the same stable adapter ID.
    #[error("adapter id `{id}` occurs more than once")]
    DuplicateId { id: AdapterId },
}

impl AdapterRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry from typed manifests.
    pub fn from_manifests(manifests: impl IntoIterator<Item = AdapterManifest>) -> Self {
        Self {
            manifests: manifests.into_iter().collect(),
        }
    }

    /// Adds one manifest.
    #[must_use]
    pub fn with_manifest(mut self, manifest: AdapterManifest) -> Self {
        self.manifests.push(manifest);
        self
    }

    /// Adds one manifest while rejecting a duplicate stable adapter ID.
    pub fn try_with_manifest(
        mut self,
        manifest: AdapterManifest,
    ) -> Result<Self, AdapterRegistryError> {
        if self.get(manifest.id().as_str()).is_some() {
            return Err(AdapterRegistryError::DuplicateId {
                id: manifest.id().clone(),
            });
        }
        self.manifests.push(manifest);
        Ok(self)
    }

    /// Looks up one manifest by id.
    pub fn get(&self, id: &str) -> Option<&AdapterManifest> {
        self.manifests
            .iter()
            .find(|manifest| manifest.id().as_str() == id)
    }

    /// Known adapter ids.
    pub fn adapter_ids(&self) -> Vec<&str> {
        self.manifests
            .iter()
            .map(|manifest| manifest.id().as_str())
            .collect()
    }

    /// All registered manifests.
    pub fn manifests(&self) -> &[AdapterManifest] {
        &self.manifests
    }
}
