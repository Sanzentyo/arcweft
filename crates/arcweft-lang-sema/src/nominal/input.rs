//! Validated final-HIR inputs and lexical scopes for nominal type resolution.

use core::hash::Hasher;
use std::{collections::BTreeMap, hash::Hash, ptr};

use arcweft_lang_hir::{
    identity::{HirSnapshotId, IdResolveError, TypeId},
    lowering::HirModuleKey,
    module::HirModule,
    project::HirProjectView,
    proof_return::{HirProofReturnHeaderModuleView, HirProofReturnHeaderProjectView},
    source_index::{HirSourcePresence, HirSourceQuery, HirSourceSite, HirTypeSourceRole},
    symbol::{
        ExternalDeclarationId, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
    },
    type_ref::HirType,
};
use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_lang_syntax::attachment::SyntaxSnapshotId;
use arcweft_source::SourceDocumentIdentity;

use crate::{
    env::TypeCheckEnv,
    registration::{
        AcceptedNominalWorld, AcceptedNominalWorldLookupError, ExternalOwnerLookupError,
    },
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

/// Semantic `Self` available at the current final-HIR type owner.
#[derive(Clone, Debug)]
pub enum SelfTypeScope {
    Absent,
    Known(TypeKind),
    Poisoned(TypePoisonId),
}

/// Deterministic fingerprint of one `Self` scope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelfTypeScopeFingerprint([u8; 32]);

/// Exact module reader used by the sole nominal resolver. The staged variant
/// borrows the paused Proof-return transaction directly; it is not a second
/// HIR database or a reconstructed header model.
#[derive(Clone, Copy)]
pub enum TypeResolutionModule<'a> {
    Published(&'a HirModule),
    ProofReturnHeader(HirProofReturnHeaderModuleView<'a, 'a>),
}

/// Exact project reader used by the sole nominal resolver.
#[derive(Clone, Copy)]
pub enum TypeResolutionProject<'a> {
    Published(HirProjectView<'a>),
    ProofReturnHeaders(HirProofReturnHeaderProjectView<'a, 'a>),
}

/// Accepted project proof or a final-HIR module resolved without project symbols.
pub enum TypeResolutionWorld<'a> {
    Accepted {
        project: TypeResolutionProject<'a>,
        symbols: &'a ProjectSymbolTable,
        environment: &'a AcceptedNominalWorld,
    },
    Detached {
        environment: &'a TypeCheckEnv,
    },
}

/// Complete validated input for the one public final-HIR resolution operation.
pub struct TypeResolutionInput<'a> {
    root: TypeId,
    module: TypeResolutionModule<'a>,
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
    StaleModuleLease {
        module: Box<CanonicalModulePath>,
        expected: HirSnapshotId,
        actual: HirSnapshotId,
    },
    SourceMismatch {
        module: Box<CanonicalModulePath>,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    InvalidTypeOwner {
        root: TypeId,
        reason: IdResolveError,
    },
    RegisteredEnvironmentIntegrity {
        external: ExternalDeclarationId,
        reason: Box<ExternalOwnerLookupError>,
    },
    RegisteredNominalIntegrity {
        external: ExternalDeclarationId,
        reason: Box<AcceptedNominalWorldLookupError>,
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
    pub fn empty() -> Self {
        let bindings = BTreeMap::new();
        let fingerprint = fingerprint_generics(&bindings);
        Self {
            bindings,
            fingerprint,
        }
    }

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
    pub const fn project(&self) -> Option<TypeResolutionProject<'a>> {
        match self {
            Self::Accepted { project, .. } => Some(*project),
            Self::Detached { .. } => None,
        }
    }

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

impl<'a> TypeResolutionProject<'a> {
    pub fn module(self, path: &CanonicalModulePath) -> Option<TypeResolutionModule<'a>> {
        match self {
            Self::Published(project) => project
                .module(path)
                .map(|module| TypeResolutionModule::Published(module.as_ref())),
            Self::ProofReturnHeaders(project) => project
                .module(path)
                .map(TypeResolutionModule::ProofReturnHeader),
        }
    }
}

impl<'a> TypeResolutionModule<'a> {
    pub fn key(self) -> &'a HirModuleKey {
        match self {
            Self::Published(module) => module.key(),
            Self::ProofReturnHeader(module) => module.key(),
        }
    }

    pub fn snapshot_id(self) -> HirSnapshotId {
        match self {
            Self::Published(module) => module.snapshot_id(),
            Self::ProofReturnHeader(module) => module.snapshot_id(),
        }
    }

    pub fn syntax_snapshot(self) -> &'a SyntaxSnapshotId {
        match self {
            Self::Published(module) => module.provenance().syntax_snapshot(),
            Self::ProofReturnHeader(module) => module.syntax_snapshot(),
        }
    }

    pub fn source_identity(self) -> &'a SourceDocumentIdentity {
        match self {
            Self::Published(module) => module.provenance().source_identity(),
            Self::ProofReturnHeader(module) => module.source_identity(),
        }
    }

    pub fn resolve_type(self, owner: TypeId) -> Result<&'a HirType, IdResolveError> {
        match self {
            Self::Published(module) => module.resolve_type(owner),
            Self::ProofReturnHeader(module) => module.resolve_type(owner),
        }
    }

    pub fn type_source_site(
        self,
        owner: TypeId,
        role: HirTypeSourceRole,
    ) -> Option<&'a HirSourceSite> {
        match self {
            Self::Published(module) => module
                .source_site(
                    module.provenance().source_identity(),
                    HirSourceQuery::Type { owner, role },
                )
                .ok()
                .and_then(|lookup| match lookup.presence() {
                    HirSourcePresence::Present(site) => Some(site),
                    HirSourcePresence::AbsentOptional => None,
                }),
            Self::ProofReturnHeader(module) => module.type_source_site(owner, role),
        }
    }

    fn same_lease(self, other: Self) -> bool {
        match (self, other) {
            (Self::Published(left), Self::Published(right)) => ptr::eq(left, right),
            (Self::ProofReturnHeader(left), Self::ProofReturnHeader(right)) => {
                left.same_transaction(right)
            }
            (Self::Published(_), Self::ProofReturnHeader(_))
            | (Self::ProofReturnHeader(_), Self::Published(_)) => false,
        }
    }
}

impl<'a> TypeResolutionInput<'a> {
    #[allow(
        clippy::too_many_arguments,
        reason = "the final accepted resolver boundary requires every proof component explicitly"
    )]
    pub fn accepted(
        root: TypeId,
        module: &'a HirModule,
        project: HirProjectView<'a>,
        symbols: &'a ProjectSymbolTable,
        environment: &'a AcceptedNominalWorld,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Result<Self, TypeResolutionInputError> {
        let module = TypeResolutionModule::Published(module);
        validate_root(root, module)?;
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
        let module_path = module.key().path();
        let accepted =
            project
                .module(module_path)
                .ok_or_else(|| TypeResolutionInputError::UnknownModule {
                    module: Box::new(module_path.clone()),
                })?;
        let accepted = TypeResolutionModule::Published(accepted.as_ref());
        if !accepted.same_lease(module) {
            return Err(TypeResolutionInputError::StaleModuleLease {
                module: Box::new(module_path.clone()),
                expected: accepted.snapshot_id(),
                actual: module.snapshot_id(),
            });
        }
        let expected = symbols.source_identity(module_path).ok_or_else(|| {
            TypeResolutionInputError::UnknownModule {
                module: Box::new(module_path.clone()),
            }
        })?;
        let actual = module.source_identity();
        if expected != actual {
            return Err(TypeResolutionInputError::SourceMismatch {
                module: Box::new(module_path.clone()),
                expected: Box::new(expected.clone()),
                actual: Box::new(actual.clone()),
            });
        }
        validate_compiled_limits(limits)?;
        Ok(Self {
            root,
            module,
            world: TypeResolutionWorld::Accepted {
                project: TypeResolutionProject::Published(project),
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
        reason = "the staged accepted resolver boundary requires every exact proof component explicitly"
    )]
    pub fn accepted_proof_return_header(
        root: TypeId,
        module: HirProofReturnHeaderModuleView<'a, 'a>,
        project: HirProofReturnHeaderProjectView<'a, 'a>,
        symbols: &'a ProjectSymbolTable,
        environment: &'a AcceptedNominalWorld,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Result<Self, TypeResolutionInputError> {
        let module = TypeResolutionModule::ProofReturnHeader(module);
        validate_root(root, module)?;
        validate_accepted_world(symbols, environment)?;
        let module_path = module.key().path();
        let accepted =
            project
                .module(module_path)
                .ok_or_else(|| TypeResolutionInputError::UnknownModule {
                    module: Box::new(module_path.clone()),
                })?;
        if !TypeResolutionModule::ProofReturnHeader(accepted).same_lease(module) {
            return Err(TypeResolutionInputError::StaleModuleLease {
                module: Box::new(module_path.clone()),
                expected: accepted.snapshot_id(),
                actual: module.snapshot_id(),
            });
        }
        validate_source_identity(module, symbols)?;
        validate_compiled_limits(limits)?;
        Ok(Self {
            root,
            module,
            world: TypeResolutionWorld::Accepted {
                project: TypeResolutionProject::ProofReturnHeaders(project),
                symbols,
                environment,
            },
            generics,
            self_scope,
            limits,
        })
    }

    pub fn detached(
        root: TypeId,
        module: &'a HirModule,
        environment: &'a TypeCheckEnv,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Result<Self, TypeResolutionInputError> {
        let module = TypeResolutionModule::Published(module);
        validate_root(root, module)?;
        validate_compiled_limits(limits)?;
        Ok(Self {
            root,
            module,
            world: TypeResolutionWorld::Detached { environment },
            generics,
            self_scope,
            limits,
        })
    }

    pub const fn root(&self) -> TypeId {
        self.root
    }

    pub const fn module(&self) -> TypeResolutionModule<'a> {
        self.module
    }

    pub fn current_module(&self) -> &'a CanonicalModulePath {
        self.module.key().path()
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

fn validate_root(
    root: TypeId,
    module: TypeResolutionModule<'_>,
) -> Result<(), TypeResolutionInputError> {
    module
        .resolve_type(root)
        .map(|_| ())
        .map_err(|reason| TypeResolutionInputError::InvalidTypeOwner { root, reason })
}

fn validate_accepted_world(
    symbols: &ProjectSymbolTable,
    environment: &AcceptedNominalWorld,
) -> Result<(), TypeResolutionInputError> {
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
    Ok(())
}

fn validate_source_identity(
    module: TypeResolutionModule<'_>,
    symbols: &ProjectSymbolTable,
) -> Result<(), TypeResolutionInputError> {
    let module_path = module.key().path();
    let expected = symbols.source_identity(module_path).ok_or_else(|| {
        TypeResolutionInputError::UnknownModule {
            module: Box::new(module_path.clone()),
        }
    })?;
    let actual = module.source_identity();
    if expected != actual {
        return Err(TypeResolutionInputError::SourceMismatch {
            module: Box::new(module_path.clone()),
            expected: Box::new(expected.clone()),
            actual: Box::new(actual.clone()),
        });
    }
    Ok(())
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
    let mut hasher = Blake3Hasher::new(b"arcweft-generic-type-scope-v2\0");
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
