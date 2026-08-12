//! Exact-key cache for accepted final-HIR type resolution.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
    sync::Arc,
};

use arcweft_lang_hir::{
    identity::{HirSnapshotId, TypeId},
    symbol::{ProjectSymbolRevision, ProjectSymbolWorldId},
    type_ref::HirTypeKind,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use crate::env::nominal::AcceptedNominalCatalogDigest;

use super::{
    GenericTypeScopeFingerprint, NominalResolutionLimits, SelfTypeScopeFingerprint,
    TypeResolutionInput, TypeResolutionInputError, TypeResolutionModule, TypeResolutionReport,
    TypeResolutionWorld,
};

/// Version of the final-HIR resolver/cache semantics represented by this key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalResolverSchemaVersion(u16);

impl NominalResolverSchemaVersion {
    /// Schema keyed exclusively by qualified final-HIR identities.
    pub const CURRENT: Self = Self(1);

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Deterministic digest of one reachable final-HIR type graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTypeStructuralDigest([u8; 32]);

impl HirTypeStructuralDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Complete accepted-world key for one checked final-HIR type identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTypeReferenceCacheKey {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    module: CanonicalModulePath,
    snapshot: HirSnapshotId,
    root: TypeId,
    structure: HirTypeStructuralDigest,
    generics: GenericTypeScopeFingerprint,
    self_scope: SelfTypeScopeFingerprint,
    catalog: AcceptedNominalCatalogDigest,
    schema: NominalResolverSchemaVersion,
    limits: NominalResolutionLimits,
}

/// Snapshot-local cache whose entries never cross an accepted key component.
#[derive(Clone, Debug, Default)]
pub struct CheckedTypeReferenceCache {
    entries: BTreeMap<CheckedTypeReferenceCacheKey, Arc<TypeResolutionReport>>,
    hits: u64,
    misses: u64,
}

impl CheckedTypeReferenceCacheKey {
    fn from_input(input: &TypeResolutionInput<'_>) -> Option<Self> {
        let TypeResolutionWorld::Accepted {
            symbols,
            environment,
            ..
        } = input.world()
        else {
            return None;
        };
        Some(Self {
            world: symbols.world().clone(),
            revision: *symbols.revision(),
            module: input.current_module().clone(),
            snapshot: input.module().snapshot_id(),
            root: input.root(),
            structure: structural_digest(input.module(), input.root()),
            generics: input.generics().fingerprint(),
            self_scope: input.self_scope().fingerprint(),
            catalog: environment.nominal_catalog().digest(),
            schema: NominalResolverSchemaVersion::CURRENT,
            limits: input.limits(),
        })
    }

    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn revision(&self) -> ProjectSymbolRevision {
        self.revision
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn root(&self) -> TypeId {
        self.root
    }

    pub const fn structure(&self) -> HirTypeStructuralDigest {
        self.structure
    }

    pub const fn generics(&self) -> GenericTypeScopeFingerprint {
        self.generics
    }

    pub const fn self_scope(&self) -> SelfTypeScopeFingerprint {
        self.self_scope
    }

    pub const fn catalog(&self) -> AcceptedNominalCatalogDigest {
        self.catalog
    }

    pub const fn schema(&self) -> NominalResolverSchemaVersion {
        self.schema
    }

    pub const fn limits(&self) -> NominalResolutionLimits {
        self.limits
    }
}

impl CheckedTypeReferenceCache {
    /// Resolves one input, reusing only an exact accepted-world report.
    pub fn resolve(
        &mut self,
        input: &TypeResolutionInput<'_>,
    ) -> Result<Arc<TypeResolutionReport>, TypeResolutionInputError> {
        let key = CheckedTypeReferenceCacheKey::from_input(input);
        if let Some(key) = key.as_ref()
            && let Some(report) = self.entries.get(key)
        {
            self.hits = self.hits.saturating_add(1);
            return Ok(Arc::clone(report));
        }

        let report = Arc::new(super::resolver::resolve_type_ref(input)?);
        if let Some(key) = key {
            self.misses = self.misses.saturating_add(1);
            self.entries.insert(key, Arc::clone(&report));
        }
        Ok(report)
    }

    pub fn entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CheckedTypeReferenceCacheKey, &TypeResolutionReport)> {
        self.entries
            .iter()
            .map(|(key, report)| (key, report.as_ref()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub const fn hits(&self) -> u64 {
        self.hits
    }

    pub const fn misses(&self) -> u64 {
        self.misses
    }
}

fn structural_digest(module: TypeResolutionModule<'_>, root: TypeId) -> HirTypeStructuralDigest {
    let mut hasher = Blake3Hasher::new(b"arcweft-final-hir-type-structure-v1\0");
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();
    while let Some(owner) = pending.pop() {
        if !seen.insert(owner) {
            continue;
        }
        let ty = module
            .resolve_type(owner)
            .expect("validated final-HIR type graph contains only live same-module IDs");
        owner.hash(&mut hasher);
        ty.kind().hash(&mut hasher);
        ty.state().hash(&mut hasher);
        push_children(ty.kind(), &mut pending);
    }
    HirTypeStructuralDigest(hasher.finalize())
}

fn push_children(kind: &HirTypeKind, pending: &mut Vec<TypeId>) {
    match kind {
        HirTypeKind::Tuple(children) | HirTypeKind::Choice(children) => {
            pending.extend(children.iter().rev().copied());
        }
        HirTypeKind::Function(function) => {
            pending.push(function.return_type());
            pending.extend(function.parameters().iter().rev().copied());
        }
        HirTypeKind::Generic(generic) => {
            pending.extend(generic.arguments().iter().rev().copied());
        }
        HirTypeKind::TraitBound(bound) => {
            pending.extend(
                bound
                    .associated()
                    .iter()
                    .rev()
                    .map(arcweft_lang_hir::type_ref::HirAssociatedTypeBinding::value),
            );
            pending.extend(bound.arguments().iter().rev().copied());
        }
        HirTypeKind::Projection(projection) => pending.push(projection.subject()),
        HirTypeKind::Reference(reference) => pending.push(reference.referent()),
        HirTypeKind::Slice(element) => pending.push(*element),
        HirTypeKind::Never
        | HirTypeKind::ConstInt(_)
        | HirTypeKind::Path(_)
        | HirTypeKind::Recovery(_) => {}
    }
}

struct Blake3Hasher(blake3::Hasher);

impl Blake3Hasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        Self(hasher)
    }

    fn finalize(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

impl Hasher for Blake3Hasher {
    fn finish(&self) -> u64 {
        let digest = self.0.clone().finalize();
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(&digest.as_bytes()[..8]);
        u64::from_le_bytes(bytes)
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::NominalResolverSchemaVersion;

    #[test]
    fn final_hir_resolver_schema_version_is_explicit() {
        assert_eq!(NominalResolverSchemaVersion::CURRENT.value(), 1);
    }
}
