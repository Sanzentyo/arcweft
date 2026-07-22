//! Validated inputs and lexical scopes for nominal type resolution.

use core::hash::Hasher;
use std::{collections::BTreeMap, hash::Hash};

use arcweft_lang_hir::symbol::{
    ExternalDeclarationId, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
    nominal::SourceBackedTypeRef,
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    types::AuthoredTypeRef,
};
use arcweft_source::SourceDocumentIdentity;

use crate::{
    env::TypeCheckEnv,
    registration::{AcceptedNominalWorld, ExternalOwnerLookupError},
    types::{GenericTypeParameterId, TypeKind, TypePoisonId},
};

use super::{
    NominalResolutionLimitKind, NominalResolutionLimits, NominalResolutionLimitsError,
    TypeSourceEvidence,
};

/// One lexical generic type binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericTypeBinding {
    id: GenericTypeParameterId,
    name: ModuleSegment,
    source: TypeSourceEvidence,
}

/// Deterministic digest of the complete generic lookup scope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericTypeScopeFingerprint([u8; 32]);

/// Duplicate generic name in one already-shadowed lexical scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericTypeScopeError {
    name: ModuleSegment,
    first: TypeSourceEvidence,
    duplicate: TypeSourceEvidence,
}

/// Immutable nearest-binding map supplied to the resolver.
#[derive(Clone, Debug)]
pub struct GenericTypeScope {
    bindings: BTreeMap<ModuleSegment, GenericTypeBinding>,
    fingerprint: GenericTypeScopeFingerprint,
}

/// Semantic `Self` available at the current authored type position.
#[derive(Clone, Debug)]
pub enum SelfTypeScope {
    Absent,
    Known(TypeKind),
    Poisoned(TypePoisonId),
}

/// Deterministic fingerprint of one `Self` scope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelfTypeScopeFingerprint([u8; 32]);

/// Accepted project proof or deliberately detached environment.
pub enum TypeResolutionWorld<'a> {
    Accepted {
        symbols: &'a ProjectSymbolTable,
        environment: &'a AcceptedNominalWorld,
    },
    Detached {
        environment: &'a TypeCheckEnv,
    },
}

/// Source carrier selected for the current resolution world.
pub enum AuthoredTypeInput<'a> {
    Accepted(&'a SourceBackedTypeRef),
    Detached(&'a AuthoredTypeRef),
}

/// Complete validated input for the one public resolution operation.
pub struct TypeResolutionInput<'a> {
    authored: AuthoredTypeInput<'a>,
    current_module: Option<&'a CanonicalModulePath>,
    world: TypeResolutionWorld<'a>,
    generics: &'a GenericTypeScope,
    self_scope: SelfTypeScope,
    limits: NominalResolutionLimits,
}

/// Infrastructure mismatch that prevents authoritative type resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeResolutionInputError {
    StaleWorld {
        symbol_world: Box<ProjectSymbolWorldId>,
        environment_world: Box<ProjectSymbolWorldId>,
    },
    StaleRevision {
        symbol_revision: ProjectSymbolRevision,
        environment_revision: ProjectSymbolRevision,
    },
    UnknownModule {
        module: Box<CanonicalModulePath>,
    },
    SourceMismatch {
        module: Box<CanonicalModulePath>,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    RegisteredEnvironmentIntegrity {
        external: ExternalDeclarationId,
        reason: Box<ExternalOwnerLookupError>,
    },
    InvalidLimits {
        reason: NominalResolutionLimitsError,
    },
}

impl GenericTypeBinding {
    pub const fn new(
        id: GenericTypeParameterId,
        name: ModuleSegment,
        source: TypeSourceEvidence,
    ) -> Self {
        Self { id, name, source }
    }

    pub const fn id(&self) -> &GenericTypeParameterId {
        &self.id
    }

    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub const fn source(&self) -> &TypeSourceEvidence {
        &self.source
    }
}

impl GenericTypeScopeFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl GenericTypeScopeError {
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub const fn first(&self) -> &TypeSourceEvidence {
        &self.first
    }

    pub const fn duplicate(&self) -> &TypeSourceEvidence {
        &self.duplicate
    }
}

impl GenericTypeScope {
    /// Empty generic scope for an authored position outside a generic owner.
    pub fn empty() -> Self {
        let bindings = BTreeMap::new();
        let fingerprint = fingerprint_generics(&bindings);
        Self {
            bindings,
            fingerprint,
        }
    }

    /// Freezes nearest visible bindings and rejects duplicate names.
    pub fn try_new(
        bindings: impl IntoIterator<Item = GenericTypeBinding>,
    ) -> Result<Self, GenericTypeScopeError> {
        let mut by_name = BTreeMap::<ModuleSegment, GenericTypeBinding>::new();
        for binding in bindings {
            if let Some(first) = by_name.get(binding.name()) {
                return Err(GenericTypeScopeError {
                    name: binding.name.clone(),
                    first: first.source.clone(),
                    duplicate: binding.source,
                });
            }
            by_name.insert(binding.name.clone(), binding);
        }
        let fingerprint = fingerprint_generics(&by_name);
        Ok(Self {
            bindings: by_name,
            fingerprint,
        })
    }

    pub fn binding(&self, name: &ModuleSegment) -> Option<&GenericTypeBinding> {
        self.bindings.get(name)
    }

    pub fn bindings(&self) -> impl ExactSizeIterator<Item = &GenericTypeBinding> {
        self.bindings.values()
    }

    pub const fn fingerprint(&self) -> GenericTypeScopeFingerprint {
        self.fingerprint
    }
}

impl Default for GenericTypeScope {
    fn default() -> Self {
        Self::empty()
    }
}

impl SelfTypeScope {
    pub fn fingerprint(&self) -> SelfTypeScopeFingerprint {
        let mut hasher = Blake3Hasher::new(b"arcweft-self-type-scope-v1\0");
        self.hash_into(&mut hasher);
        SelfTypeScopeFingerprint(hasher.finalize())
    }

    fn hash_into(&self, state: &mut impl Hasher) {
        match self {
            Self::Absent => state.write_u8(0),
            Self::Known(ty) => {
                state.write_u8(1);
                ty.hash(state);
            }
            Self::Poisoned(poison) => {
                state.write_u8(2);
                poison.hash(state);
            }
        }
    }
}

impl SelfTypeScopeFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl<'a> TypeResolutionWorld<'a> {
    pub const fn symbols(&self) -> Option<&'a ProjectSymbolTable> {
        match self {
            Self::Accepted { symbols, .. } => Some(*symbols),
            Self::Detached { .. } => None,
        }
    }

    pub fn environment(&self) -> &'a TypeCheckEnv {
        match self {
            Self::Accepted { environment, .. } => environment.typecheck_env(),
            Self::Detached { environment } => environment,
        }
    }

    pub const fn accepted_environment(&self) -> Option<&'a AcceptedNominalWorld> {
        match self {
            Self::Accepted { environment, .. } => Some(*environment),
            Self::Detached { .. } => None,
        }
    }
}

impl<'a> AuthoredTypeInput<'a> {
    pub const fn authored(&self) -> &'a AuthoredTypeRef {
        match self {
            Self::Accepted(source_backed) => source_backed.authored(),
            Self::Detached(authored) => authored,
        }
    }

    pub const fn source_backed(&self) -> Option<&'a SourceBackedTypeRef> {
        match self {
            Self::Accepted(source_backed) => Some(*source_backed),
            Self::Detached(_) => None,
        }
    }
}

impl<'a> TypeResolutionInput<'a> {
    #[allow(
        clippy::too_many_arguments,
        reason = "the final accepted resolver boundary requires every proof component explicitly"
    )]
    pub fn accepted(
        authored: &'a SourceBackedTypeRef,
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        environment: &'a AcceptedNominalWorld,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Result<Self, TypeResolutionInputError> {
        if symbols.world() != environment.world() {
            return Err(TypeResolutionInputError::StaleWorld {
                symbol_world: Box::new(symbols.world().clone()),
                environment_world: Box::new(environment.world().clone()),
            });
        }
        if symbols.revision() != environment.symbol_revision() {
            return Err(TypeResolutionInputError::StaleRevision {
                symbol_revision: *symbols.revision(),
                environment_revision: *environment.symbol_revision(),
            });
        }
        let expected = symbols.source_identity(current_module).ok_or_else(|| {
            TypeResolutionInputError::UnknownModule {
                module: Box::new(current_module.clone()),
            }
        })?;
        for (_, source) in authored.spans().nodes() {
            for span in core::iter::once(source.whole()).chain(
                source
                    .head()
                    .map(arcweft_lang_syntax::types::TypeRefHeadSource::range),
            ) {
                if span.source() != expected {
                    return Err(TypeResolutionInputError::SourceMismatch {
                        module: Box::new(current_module.clone()),
                        expected: Box::new(expected.clone()),
                        actual: Box::new(span.source().clone()),
                    });
                }
            }
        }
        validate_compiled_limits(limits)?;
        Ok(Self {
            authored: AuthoredTypeInput::Accepted(authored),
            current_module: Some(current_module),
            world: TypeResolutionWorld::Accepted {
                symbols,
                environment,
            },
            generics,
            self_scope,
            limits,
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "detached resolution still requires every lexical and resource boundary explicitly"
    )]
    pub const fn detached(
        authored: &'a AuthoredTypeRef,
        current_module: Option<&'a CanonicalModulePath>,
        environment: &'a TypeCheckEnv,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Self {
        Self {
            authored: AuthoredTypeInput::Detached(authored),
            current_module,
            world: TypeResolutionWorld::Detached { environment },
            generics,
            self_scope,
            limits,
        }
    }

    pub const fn authored(&self) -> &AuthoredTypeInput<'a> {
        &self.authored
    }

    pub const fn current_module(&self) -> Option<&'a CanonicalModulePath> {
        self.current_module
    }

    pub const fn world(&self) -> &TypeResolutionWorld<'a> {
        &self.world
    }

    pub const fn generics(&self) -> &'a GenericTypeScope {
        self.generics
    }

    pub const fn self_scope(&self) -> &SelfTypeScope {
        &self.self_scope
    }

    pub const fn limits(&self) -> NominalResolutionLimits {
        self.limits
    }
}

fn validate_compiled_limits(
    limits: NominalResolutionLimits,
) -> Result<(), TypeResolutionInputError> {
    let production = NominalResolutionLimits::PRODUCTION;
    let values = [
        (
            NominalResolutionLimitKind::TypeNodesPerReference,
            limits.type_nodes_per_reference(),
            production.type_nodes_per_reference(),
        ),
        (
            NominalResolutionLimitKind::RecursiveTypeDepth,
            u64::from(limits.recursive_type_depth()),
            u64::from(production.recursive_type_depth()),
        ),
        (
            NominalResolutionLimitKind::GenericArgumentsPerApplication,
            u64::from(limits.generic_arguments_per_application()),
            u64::from(production.generic_arguments_per_application()),
        ),
        (
            NominalResolutionLimitKind::AliasExpansionDepth,
            u64::from(limits.alias_expansion_depth()),
            u64::from(production.alias_expansion_depth()),
        ),
        (
            NominalResolutionLimitKind::AliasExpansionNodes,
            limits.alias_expansion_nodes(),
            production.alias_expansion_nodes(),
        ),
        (
            NominalResolutionLimitKind::DiagnosticsPerTypeReference,
            u64::from(limits.diagnostics_per_type_reference()),
            u64::from(production.diagnostics_per_type_reference()),
        ),
        (
            NominalResolutionLimitKind::RelatedLabelsPerDiagnostic,
            u64::from(limits.related_labels_per_diagnostic()),
            u64::from(production.related_labels_per_diagnostic()),
        ),
        (
            NominalResolutionLimitKind::WorkPerReference,
            limits.work_per_reference(),
            production.work_per_reference(),
        ),
    ];
    values
        .into_iter()
        .find(|(_, value, ceiling)| value > ceiling)
        .map_or(Ok(()), |(kind, value, ceiling)| {
            Err(TypeResolutionInputError::InvalidLimits {
                reason: NominalResolutionLimitsError::AboveHardCeiling {
                    kind,
                    value,
                    ceiling,
                },
            })
        })
}

fn fingerprint_generics(
    bindings: &BTreeMap<ModuleSegment, GenericTypeBinding>,
) -> GenericTypeScopeFingerprint {
    let mut hasher = Blake3Hasher::new(b"arcweft-generic-type-scope-v1\0");
    hasher.write_usize(bindings.len());
    for (name, binding) in bindings {
        name.hash(&mut hasher);
        binding.id.hash(&mut hasher);
        hasher.write_usize(binding.source.local().start());
        hasher.write_usize(binding.source.local().end());
        binding.source.project().hash(&mut hasher);
    }
    GenericTypeScopeFingerprint(hasher.finalize())
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
    use arcweft_lang_syntax::ast::{common::TextRange, module_path::ModuleSegment};

    use crate::types::{DetachedTypeOwnerId, GenericTypeOwnerId};

    use super::*;

    fn binding(name: &str, ordinal: u16, start: usize) -> GenericTypeBinding {
        GenericTypeBinding::new(
            GenericTypeParameterId::new(
                GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(9)),
                ordinal,
            ),
            ModuleSegment::new(name).expect("generic name"),
            TypeSourceEvidence::new(TextRange::new(start, start + name.len()), None),
        )
    }

    #[test]
    fn generic_scope_fingerprint_is_insertion_order_independent() {
        let first =
            GenericTypeScope::try_new([binding("T", 0, 0), binding("E", 1, 3)]).expect("scope");
        let second =
            GenericTypeScope::try_new([binding("E", 1, 3), binding("T", 0, 0)]).expect("scope");
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.bindings().len(), 2);
    }

    #[test]
    fn generic_scope_rejects_duplicate_nearest_bindings() {
        let error = GenericTypeScope::try_new([binding("T", 0, 0), binding("T", 1, 5)])
            .expect_err("duplicate generic must fail");
        assert_eq!(error.name().as_str(), "T");
        assert_eq!(error.first().local(), TextRange::new(0, 1));
        assert_eq!(error.duplicate().local(), TextRange::new(5, 6));
    }

    #[test]
    fn self_scope_fingerprint_distinguishes_absent_known_and_poisoned() {
        assert_ne!(
            SelfTypeScope::Absent.fingerprint(),
            SelfTypeScope::Known(TypeKind::Bool).fingerprint()
        );
        assert_ne!(
            SelfTypeScope::Known(TypeKind::Bool).fingerprint(),
            SelfTypeScope::Poisoned(TypePoisonId::from_index(1)).fingerprint()
        );
    }
}
