use core::fmt;
use std::collections::BTreeMap;

use arcweft_id::{DeclarationIdentityFamily, DeclarationName, PublicId};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathError, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, SymbolPath},
};
use arcweft_source::{
    SourceDocumentId, SourceDocumentIdentity, SourceSetRevision, SourceSetRevisionError, SourceSpan,
};
use thiserror::Error;

use crate::{
    identity::{HirSnapshotId, ItemId},
    source_index::HirCallableSourceOwner,
};

use super::{nominal::ProjectNominalDeclaration, qualified_name};

/// Canonical package component of a source declaration identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallablePackageId(String);

/// Identity of an original callable declaration before imports or re-exports.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    owner: CallableDeclarationOwner,
    owner_path: Vec<ModuleSegment>,
    name: String,
}

/// Structural identity of one project Trait declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: ModuleSegment,
}

/// Structural identity of one source-ordered project Impl declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImplDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    source_ordinal: u32,
}

/// Structural identity of one callable requirement owned by a Trait.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitMethodRequirementId {
    trait_declaration: TraitDeclarationId,
    method: ModuleSegment,
}

/// Semantic family of one method declared by an Impl.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ImplMethodKind {
    Trait,
    Inherent,
}

/// Structural identity of one callable method owned by an Impl.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImplMethodDeclarationId {
    implementation: ImplDeclarationId,
    kind: ImplMethodKind,
    method: ModuleSegment,
}

/// Publication origin of one accepted Flow identity.
///
/// Name-derived and empty-marker identities remain module-scoped even though
/// their canonical public spelling is retained. Authored absolute IDs are
/// project-global and participate in duplicate-public-ID rejection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FlowPublicationKind {
    ModuleScoped,
    AuthoredAbsolute,
}

/// Structural identity of one accepted Flow execution owner.
///
/// Flow is intentionally not an ordinary callable declaration. The project
/// transaction derives or validates `public_id` exactly once and retains that
/// typed result together with its module identity for every later consumer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    public_id: PublicId,
    publication: FlowPublicationKind,
}

/// Sole structural key for a project-owned callable declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDeclarationKey {
    Existing(CallableDeclarationId),
    TraitRequirement(TraitMethodRequirementId),
    ImplMethod(ImplMethodDeclarationId),
    Flow(FlowDeclarationId),
}

/// Durable digest of one structural project callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableDeclarationDigest([u8; 32]);

/// Session-only identity of one registered Proof in one exact HIR snapshot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProofArtifactId {
    declaration: CallableDeclarationId,
    snapshot: HirSnapshotId,
    item: ItemId,
}

/// Source declaration family that owns a callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDeclarationOwner {
    Function,
    ExternCapability,
    View,
    Predicate,
    Proof,
    TraitRequirement,
    TraitImplementation,
    InherentMethod,
    /// Structural executable-body owner for an authored Flow.
    ///
    /// Flow identities are not published into the ordinary callable symbol
    /// namespace. This owner exists so downstream artifact identities can use
    /// the same canonical declaration vocabulary without inventing a second
    /// stringly identity model.
    Flow,
}

impl CallableDeclarationOwner {
    /// Stable source-family label used at serialization and tooling boundaries.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::ExternCapability => "extern_capability",
            Self::View => "view",
            Self::Predicate => "predicate",
            Self::Proof => "proof",
            Self::TraitRequirement => "trait_requirement",
            Self::TraitImplementation => "trait_implementation",
            Self::InherentMethod => "inherent_method",
            Self::Flow => "flow",
        }
    }

    /// Whether declarations of this family can become runtime call targets.
    pub const fn is_runtime_callable(self) -> bool {
        matches!(
            self,
            Self::Function
                | Self::ExternCapability
                | Self::TraitImplementation
                | Self::InherentMethod
        )
    }

    /// Whether this declaration owns executable HIR that runtime reachability
    /// must close. Extern capabilities are runtime-callable host boundaries,
    /// but deliberately do not own an Arcweft executable body.
    pub const fn owns_runtime_executable_body(self) -> bool {
        matches!(
            self,
            Self::Function | Self::TraitImplementation | Self::InherentMethod | Self::Flow
        )
    }

    /// Whether declarations of this family denote logical Boolean callables.
    pub const fn is_logical_callable(self) -> bool {
        matches!(self, Self::Predicate)
    }

    /// Whether declarations of this family can be invoked as proof statements.
    pub const fn permits_proof_statement_call(self) -> bool {
        matches!(self, Self::Proof)
    }

    /// Whether this owner belongs to the structural method family.
    pub const fn is_method(self) -> bool {
        matches!(
            self,
            Self::TraitRequirement | Self::TraitImplementation | Self::InherentMethod
        )
    }

    /// Whether this declaration is a dispatch contract rather than executable code.
    pub const fn is_dispatch_contract(self) -> bool {
        matches!(self, Self::TraitRequirement)
    }

    /// Canonical structural digest tag. Existing tags are append-only.
    pub const fn digest_tag(self) -> u8 {
        match self {
            Self::Function => 0,
            Self::ExternCapability => 1,
            Self::View => 2,
            Self::Predicate => 3,
            Self::Proof => 4,
            Self::TraitRequirement => 5,
            Self::TraitImplementation => 6,
            Self::InherentMethod => 7,
            Self::Flow => 8,
        }
    }
}

/// One callable declaration indexed independently from every source alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSymbol {
    pub(super) declaration: CallableDeclarationKey,
    pub(super) visibility: Option<Visibility>,
    pub(super) fx: bool,
    pub(super) source_snapshot: HirSnapshotId,
    pub(super) source_item: ItemId,
    pub(super) source_owner: HirCallableSourceOwner,
    pub(super) declaration_span: SourceSpan,
    pub(super) name_span: SourceSpan,
    pub(super) executable: bool,
}

/// Table-local handle assigned to one validated external seed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeclarationSeedId(u32);

/// Table-local handle assigned to one linked external declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeclarationId(u32);

/// Complete identity of one project-symbol world.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectSymbolWorldId {
    package: CallablePackageId,
    root_document: SourceDocumentId,
    profile: String,
}

/// Revision of every source document that can affect project symbols.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectSymbolRevision(SourceSetRevision);

/// One direct source-visible binding for an external declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectDirectBinding {
    module: CanonicalModulePath,
    path: ProjectSymbolPath,
    visibility: Option<Visibility>,
    source: SourceSpan,
    authored_alias: bool,
}

/// Validated external declaration before table-local IDs are assigned.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeclarationSeed {
    canonical_path: SymbolPath,
    visibility: Option<Visibility>,
    declaration: SourceSpan,
    direct_bindings: Vec<ProjectDirectBinding>,
}

/// Complete source-revision-bound set of external declaration seeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectExternalDeclarations {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    declarations: BTreeMap<ExternalDeclarationSeedId, ExternalDeclarationSeed>,
}

/// External declaration stored by the unified project symbol table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalSymbol {
    declaration: ExternalDeclarationId,
    canonical_path: SymbolPath,
    visibility: Option<Visibility>,
    declaration_span: SourceSpan,
}

/// One authored retained-identity declaration published by the project table.
///
/// The semantic public ID is the project-wide key. The local declaration name
/// remains a module-scope binding and is deliberately not used to reconstruct
/// the public identity after publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRetainedSymbol {
    public_id: PublicId,
    family: DeclarationIdentityFamily,
    name: DeclarationName,
    owner: ItemId,
    module: CanonicalModulePath,
    visibility: Option<Visibility>,
    declaration_span: SourceSpan,
    executable: bool,
}

/// Unified declaration identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectDeclarationId {
    Callable(CallableDeclarationKey),
    Trait(TraitDeclarationId),
    Impl(ImplDeclarationId),
    External(ExternalDeclarationId),
    Nominal(super::nominal::ProjectNominalDeclarationId),
    Retained(PublicId),
}

/// Unified project declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectSymbol {
    Callable(CallableSymbol),
    External(ExternalSymbol),
    Nominal(Box<ProjectNominalDeclaration>),
    Retained(ProjectRetainedSymbol),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallablePackageIdError {
    #[error("callable package identity cannot be empty")]
    Empty,
    #[error("callable package identity must contain only letters, digits, `_`, `-`, or `.`")]
    Invalid,
}

#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDeclarationIdError {
    #[error("callable `{name}` has no canonical declaration module")]
    MissingModule { name: String },
    #[error(transparent)]
    InvalidName(#[from] ModulePathError),
    #[error("method callable owners require a structural Trait/Impl method key")]
    MethodOwnerRequiresStructuralKey,
    #[error("Flow execution owners require a structural Flow declaration key")]
    FlowOwnerRequiresStructuralKey,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProofArtifactIdentityError {
    #[error("callable is not registered in this project symbol table")]
    UnknownDeclaration { declaration: CallableDeclarationId },
    #[error("callable is not a proof declaration")]
    NotProof {
        declaration: CallableDeclarationId,
        actual: CallableDeclarationOwner,
    },
    #[error("proof HIR snapshot is not present in the supplied project view")]
    SnapshotUnavailable { snapshot: HirSnapshotId },
    #[error("registered proof source does not resolve to a proof item")]
    ItemMismatch {
        snapshot: HirSnapshotId,
        item: ItemId,
    },
    #[error("registered proof source does not match the table declaration")]
    RegistrationMismatch { declaration: CallableDeclarationId },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSymbolWorldIdError {
    #[error("project profile identity must not be empty")]
    EmptyProfile,
    #[error("project profile identity contains a control character at byte {byte}")]
    ProfileControl { byte: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExternalDeclarationSeedError {
    #[error("external declaration `{canonical_path}` has no direct binding")]
    MissingDirectBinding {
        canonical_path: SymbolPath,
        declaration: SourceSpan,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectExternalDeclarationsError {
    #[error("external declaration count does not fit u32")]
    SeedCountOverflow { count: usize },
}

/// Invalid source-visible binding supplied directly by an external producer.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectDirectBindingError {
    #[error("direct project binding path must use the implicit project root, found {root:?}")]
    ExplicitRoot { root: ModulePathRoot },
}

impl CallablePackageId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CallablePackageIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CallablePackageIdError::Empty);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
        {
            return Err(CallablePackageIdError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CallableDeclarationId {
    pub fn try_new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        owner: CallableDeclarationOwner,
        name: impl Into<String>,
    ) -> Result<Self, CallableDeclarationIdError> {
        if owner.is_method() {
            return Err(CallableDeclarationIdError::MethodOwnerRequiresStructuralKey);
        }
        if owner == CallableDeclarationOwner::Flow {
            return Err(CallableDeclarationIdError::FlowOwnerRequiresStructuralKey);
        }
        let name = name.into();
        arcweft_lang_syntax::ast::module_path::ModuleSegment::new(name.clone())?;
        Ok(Self {
            package,
            module,
            owner,
            owner_path: Vec::new(),
            name,
        })
    }

    /// Creates a callable owned by a typed declaration path inside one source
    /// module, such as `extern capability fs { fn read_text(...) }`.
    pub fn try_new_in_owner_path(
        package: CallablePackageId,
        module: CanonicalModulePath,
        owner: CallableDeclarationOwner,
        owner_path: impl IntoIterator<Item = ModuleSegment>,
        name: impl Into<String>,
    ) -> Result<Self, CallableDeclarationIdError> {
        let mut id = Self::try_new(package, module, owner, name)?;
        id.owner_path = owner_path.into_iter().collect();
        Ok(id)
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn owner(&self) -> CallableDeclarationOwner {
        self.owner
    }

    pub fn owner_path(&self) -> &[ModuleSegment] {
        &self.owner_path
    }

    /// Returns the final segment of this accepted non-empty public ID.
    ///
    /// # Panics
    ///
    /// Panics only if construction invariants were violated and the accepted
    /// public ID has no segment.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn qualified_name(&self) -> String {
        let owner = self
            .owner_path
            .iter()
            .map(ModuleSegment::as_str)
            .chain(std::iter::once(self.name()))
            .collect::<Vec<_>>()
            .join(".");
        qualified_name(&self.module, &owner)
    }
}

impl TraitDeclarationId {
    pub(crate) fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        name: ModuleSegment,
    ) -> Self {
        Self {
            package,
            module,
            name,
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }
}

impl ImplDeclarationId {
    pub(crate) const fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        source_ordinal: u32,
    ) -> Self {
        Self {
            package,
            module,
            source_ordinal,
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }
}

impl TraitMethodRequirementId {
    pub(crate) const fn new(trait_declaration: TraitDeclarationId, method: ModuleSegment) -> Self {
        Self {
            trait_declaration,
            method,
        }
    }

    pub const fn trait_declaration(&self) -> &TraitDeclarationId {
        &self.trait_declaration
    }

    pub const fn method(&self) -> &ModuleSegment {
        &self.method
    }
}

impl ImplMethodKind {
    pub const fn owner(self) -> CallableDeclarationOwner {
        match self {
            Self::Trait => CallableDeclarationOwner::TraitImplementation,
            Self::Inherent => CallableDeclarationOwner::InherentMethod,
        }
    }

    pub const fn digest_tag(self) -> u8 {
        match self {
            Self::Trait => 0,
            Self::Inherent => 1,
        }
    }
}

impl ImplMethodDeclarationId {
    pub(crate) const fn new(
        implementation: ImplDeclarationId,
        kind: ImplMethodKind,
        method: ModuleSegment,
    ) -> Self {
        Self {
            implementation,
            kind,
            method,
        }
    }

    pub const fn implementation(&self) -> &ImplDeclarationId {
        &self.implementation
    }

    pub const fn kind(&self) -> ImplMethodKind {
        self.kind
    }

    pub const fn method(&self) -> &ModuleSegment {
        &self.method
    }
}

impl FlowDeclarationId {
    pub(crate) const fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        public_id: PublicId,
        publication: FlowPublicationKind,
    ) -> Self {
        Self {
            package,
            module,
            public_id,
            publication,
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }

    pub const fn publication(&self) -> FlowPublicationKind {
        self.publication
    }

    /// Returns the terminal public-label component.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted Flow identity violates the constructor's
    /// non-empty public-ID invariant.
    pub fn name(&self) -> &str {
        self.public_id
            .as_str()
            .rsplit('.')
            .next()
            .expect("accepted Flow public IDs are non-empty")
    }

    /// Durable module-preserving identity used after the accepted project
    /// transaction. Public spelling remains presentation metadata and must not
    /// be parsed back into this identity.
    pub fn semantic_digest(&self) -> CallableDeclarationDigest {
        CallableDeclarationKey::Flow(self.clone()).semantic_digest()
    }
}

impl CallableDeclarationKey {
    pub const fn owner(&self) -> CallableDeclarationOwner {
        match self {
            Self::Existing(declaration) => declaration.owner(),
            Self::TraitRequirement(_) => CallableDeclarationOwner::TraitRequirement,
            Self::ImplMethod(declaration) => declaration.kind().owner(),
            Self::Flow(_) => CallableDeclarationOwner::Flow,
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        match self {
            Self::Existing(declaration) => declaration.package(),
            Self::TraitRequirement(declaration) => declaration.trait_declaration().package(),
            Self::ImplMethod(declaration) => declaration.implementation().package(),
            Self::Flow(declaration) => declaration.package(),
        }
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        match self {
            Self::Existing(declaration) => declaration.module(),
            Self::TraitRequirement(declaration) => declaration.trait_declaration().module(),
            Self::ImplMethod(declaration) => declaration.implementation().module(),
            Self::Flow(declaration) => declaration.module(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Existing(declaration) => declaration.name(),
            Self::TraitRequirement(declaration) => declaration.method().as_str(),
            Self::ImplMethod(declaration) => declaration.method().as_str(),
            Self::Flow(declaration) => declaration.name(),
        }
    }

    /// Returns a non-authoritative qualified display name for tooling.
    ///
    /// Structural lookup and persistence use this key or its semantic digest;
    /// this spelling is never parsed back into an identity.
    pub fn qualified_name(&self) -> String {
        match self {
            Self::Existing(declaration) => declaration.qualified_name(),
            Self::TraitRequirement(declaration) => {
                let owner = declaration.trait_declaration();
                let member = format!(
                    "{}.{}",
                    owner.name().as_str(),
                    declaration.method().as_str()
                );
                qualified_name(owner.module(), &member)
            }
            Self::ImplMethod(declaration) => {
                let owner = declaration.implementation();
                let member = format!(
                    "impl#{}.{}",
                    owner.source_ordinal(),
                    declaration.method().as_str()
                );
                qualified_name(owner.module(), &member)
            }
            Self::Flow(declaration) => {
                qualified_name(declaration.module(), declaration.public_id().as_str())
            }
        }
    }

    pub fn semantic_digest(&self) -> CallableDeclarationDigest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.callable-declaration.v1\0");
        match self {
            Self::Existing(declaration) => {
                hasher.update(&[0]);
                digest_string(&mut hasher, declaration.package().as_str());
                digest_segments(&mut hasher, declaration.module().segments());
                hasher.update(&[declaration.owner().digest_tag()]);
                digest_segments(&mut hasher, declaration.owner_path());
                digest_string(&mut hasher, declaration.name());
            }
            Self::TraitRequirement(declaration) => {
                hasher.update(&[1]);
                let owner = declaration.trait_declaration();
                digest_string(&mut hasher, owner.package().as_str());
                digest_segments(&mut hasher, owner.module().segments());
                digest_string(&mut hasher, owner.name().as_str());
                digest_string(&mut hasher, declaration.method().as_str());
            }
            Self::ImplMethod(declaration) => {
                hasher.update(&[2]);
                let owner = declaration.implementation();
                digest_string(&mut hasher, owner.package().as_str());
                digest_segments(&mut hasher, owner.module().segments());
                hasher.update(&owner.source_ordinal().to_le_bytes());
                hasher.update(&[declaration.kind().digest_tag()]);
                digest_string(&mut hasher, declaration.method().as_str());
            }
            Self::Flow(declaration) => {
                hasher.update(&[3]);
                digest_string(&mut hasher, declaration.package().as_str());
                digest_segments(&mut hasher, declaration.module().segments());
                digest_string(&mut hasher, declaration.public_id().as_str());
                hasher.update(&[match declaration.publication() {
                    FlowPublicationKind::ModuleScoped => 0,
                    FlowPublicationKind::AuthoredAbsolute => 1,
                }]);
            }
        }
        CallableDeclarationDigest(*hasher.finalize().as_bytes())
    }
}

impl CallableDeclarationDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for CallableDeclarationDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl ProofArtifactId {
    pub(super) const fn new(
        declaration: CallableDeclarationId,
        snapshot: HirSnapshotId,
        item: ItemId,
    ) -> Self {
        Self {
            declaration,
            snapshot,
            item,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn item(&self) -> ItemId {
        self.item
    }
}

fn digest_segments(hasher: &mut blake3::Hasher, segments: &[ModuleSegment]) {
    digest_len(hasher, segments.len());
    for segment in segments {
        digest_string(hasher, segment.as_str());
    }
}

fn digest_string(hasher: &mut blake3::Hasher, value: &str) {
    digest_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn digest_len(hasher: &mut blake3::Hasher, length: usize) {
    let length = u32::try_from(length).expect("accepted callable identity lengths fit u32");
    hasher.update(&length.to_le_bytes());
}

impl CallableSymbol {
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn owner(&self) -> CallableDeclarationOwner {
        self.declaration.owner()
    }

    pub const fn is_fx(&self) -> bool {
        self.fx
    }

    pub const fn source_snapshot(&self) -> HirSnapshotId {
        self.source_snapshot
    }

    pub const fn source_item(&self) -> ItemId {
        self.source_item
    }

    pub const fn source_owner(&self) -> HirCallableSourceOwner {
        self.source_owner
    }

    pub const fn declaration_span(&self) -> &SourceSpan {
        &self.declaration_span
    }

    pub const fn name_span(&self) -> &SourceSpan {
        &self.name_span
    }

    pub const fn is_executable(&self) -> bool {
        self.executable
    }

    pub fn is_visible_from(&self, requester: &CanonicalModulePath) -> bool {
        if requester == self.declaration.module() {
            return true;
        }
        match self.visibility {
            Some(Visibility::Public | Visibility::Crate) => true,
            Some(Visibility::Super) => {
                let parent = self
                    .declaration
                    .module()
                    .parent()
                    .unwrap_or_else(CanonicalModulePath::crate_root);
                requester.segments().starts_with(parent.segments())
            }
            None => false,
        }
    }
}

impl ExternalDeclarationSeedId {
    pub(super) const fn from_index(index: u32) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> u32 {
        self.0
    }
}

impl ExternalDeclarationId {
    pub(super) const fn from_index(index: u32) -> Self {
        Self(index)
    }
}

impl ProjectSymbolWorldId {
    pub fn try_new(
        package: CallablePackageId,
        root_document: SourceDocumentId,
        profile: impl Into<String>,
    ) -> Result<Self, ProjectSymbolWorldIdError> {
        let profile = profile.into();
        if profile.is_empty() {
            return Err(ProjectSymbolWorldIdError::EmptyProfile);
        }
        if let Some((byte, _)) = profile
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(ProjectSymbolWorldIdError::ProfileControl { byte });
        }
        Ok(Self {
            package,
            root_document,
            profile,
        })
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn root_document(&self) -> &SourceDocumentId {
        &self.root_document
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }
}

impl ProjectSymbolRevision {
    pub fn try_for_documents<'a>(
        documents: impl IntoIterator<Item = &'a SourceDocumentIdentity>,
    ) -> Result<Self, SourceSetRevisionError> {
        SourceSetRevision::try_for_identities(documents).map(Self)
    }

    pub const fn as_source_set(&self) -> &SourceSetRevision {
        &self.0
    }
}

impl ProjectDirectBinding {
    pub fn try_new(
        module: CanonicalModulePath,
        path: ProjectSymbolPath,
        visibility: Option<Visibility>,
        source: SourceSpan,
        authored_alias: bool,
    ) -> Result<Self, ProjectDirectBindingError> {
        if path.root() != ModulePathRoot::ImplicitCrate {
            return Err(ProjectDirectBindingError::ExplicitRoot { root: path.root() });
        }
        Ok(Self {
            module,
            path,
            visibility,
            source,
            authored_alias,
        })
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn path(&self) -> &ProjectSymbolPath {
        &self.path
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn authored_alias(&self) -> bool {
        self.authored_alias
    }
}

impl ExternalDeclarationSeed {
    pub fn try_new(
        canonical_path: SymbolPath,
        visibility: Option<Visibility>,
        declaration: SourceSpan,
        mut direct_bindings: Vec<ProjectDirectBinding>,
    ) -> Result<Self, ExternalDeclarationSeedError> {
        if direct_bindings.is_empty() {
            return Err(ExternalDeclarationSeedError::MissingDirectBinding {
                canonical_path,
                declaration,
            });
        }
        direct_bindings.sort();
        direct_bindings.dedup();
        Ok(Self {
            canonical_path,
            visibility,
            declaration,
            direct_bindings,
        })
    }

    pub const fn canonical_path(&self) -> &SymbolPath {
        &self.canonical_path
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn declaration(&self) -> &SourceSpan {
        &self.declaration
    }

    pub fn direct_bindings(&self) -> &[ProjectDirectBinding] {
        &self.direct_bindings
    }
}

impl ProjectExternalDeclarations {
    pub fn try_new(
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
        mut seeds: Vec<ExternalDeclarationSeed>,
    ) -> Result<Self, ProjectExternalDeclarationsError> {
        seeds.sort();
        seeds.dedup();
        let count = seeds.len();
        if u32::try_from(count).is_err() {
            return Err(ProjectExternalDeclarationsError::SeedCountOverflow { count });
        }
        let declarations = seeds
            .into_iter()
            .enumerate()
            .map(|(index, seed)| {
                let index = u32::try_from(index)
                    .map_err(|_| ProjectExternalDeclarationsError::SeedCountOverflow { count })?;
                Ok((ExternalDeclarationSeedId::from_index(index), seed))
            })
            .collect::<Result<_, ProjectExternalDeclarationsError>>()?;
        Ok(Self {
            world,
            revision,
            declarations,
        })
    }

    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn revision(&self) -> &ProjectSymbolRevision {
        &self.revision
    }

    pub fn declarations(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExternalDeclarationSeedId, &ExternalDeclarationSeed)> {
        self.declarations.iter().map(|(id, seed)| (*id, seed))
    }

    pub fn declaration(&self, id: ExternalDeclarationSeedId) -> Option<&ExternalDeclarationSeed> {
        self.declarations.get(&id)
    }

    pub fn seed_id(&self, seed: &ExternalDeclarationSeed) -> Option<ExternalDeclarationSeedId> {
        self.declarations
            .iter()
            .find_map(|(id, candidate)| (candidate == seed).then_some(*id))
    }
}

impl ExternalSymbol {
    pub(super) fn new(declaration: ExternalDeclarationId, seed: &ExternalDeclarationSeed) -> Self {
        Self {
            declaration,
            canonical_path: seed.canonical_path().clone(),
            visibility: seed.visibility(),
            declaration_span: seed.declaration().clone(),
        }
    }

    pub const fn declaration(&self) -> ExternalDeclarationId {
        self.declaration
    }

    pub const fn canonical_path(&self) -> &SymbolPath {
        &self.canonical_path
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn declaration_span(&self) -> &SourceSpan {
        &self.declaration_span
    }
}

impl ProjectRetainedSymbol {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        public_id: PublicId,
        family: DeclarationIdentityFamily,
        name: DeclarationName,
        owner: ItemId,
        module: CanonicalModulePath,
        visibility: Option<Visibility>,
        declaration_span: SourceSpan,
        executable: bool,
    ) -> Self {
        Self {
            public_id,
            family,
            name,
            owner,
            module,
            visibility,
            declaration_span,
            executable,
        }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }

    pub const fn family(&self) -> DeclarationIdentityFamily {
        self.family
    }

    pub const fn name(&self) -> &DeclarationName {
        &self.name
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn declaration_span(&self) -> &SourceSpan {
        &self.declaration_span
    }

    pub const fn is_executable(&self) -> bool {
        self.executable
    }

    pub fn is_visible_from(&self, requester: &CanonicalModulePath) -> bool {
        if requester == &self.module {
            return true;
        }
        match self.visibility {
            Some(Visibility::Public | Visibility::Crate) => true,
            Some(Visibility::Super) => {
                let parent = self
                    .module
                    .parent()
                    .unwrap_or_else(CanonicalModulePath::crate_root);
                requester.segments().starts_with(parent.segments())
            }
            None => false,
        }
    }
}

impl fmt::Display for CallablePackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for CallableDeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}", self.package, self.qualified_name())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CallableDeclarationId, CallableDeclarationOwner, CallablePackageId, ProjectSymbolRevision,
        ProjectSymbolWorldId,
    };
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    #[test]
    fn declaration_identity_uses_original_package_and_module() {
        let package = CallablePackageId::try_new("opening-game").unwrap();
        let module = CanonicalModulePath::crate_root()
            .join(ModuleSegment::new("view").unwrap())
            .join(ModuleSegment::new("effects").unwrap());
        let id = CallableDeclarationId::try_new(
            package,
            module,
            CallableDeclarationOwner::Function,
            "notice",
        )
        .unwrap();
        assert_eq!(id.owner(), CallableDeclarationOwner::Function);
        assert_eq!(id.qualified_name(), "view.effects.notice");
        assert_eq!(id.to_string(), "opening-game::view.effects.notice");
    }

    #[test]
    fn callable_owner_owns_runtime_logical_and_proof_call_policy() {
        assert!(CallableDeclarationOwner::Function.is_runtime_callable());
        assert!(!CallableDeclarationOwner::Function.is_logical_callable());
        assert!(!CallableDeclarationOwner::Function.permits_proof_statement_call());

        assert!(!CallableDeclarationOwner::Predicate.is_runtime_callable());
        assert!(CallableDeclarationOwner::Predicate.is_logical_callable());
        assert!(!CallableDeclarationOwner::Predicate.permits_proof_statement_call());

        assert!(!CallableDeclarationOwner::Proof.is_runtime_callable());
        assert!(!CallableDeclarationOwner::Proof.is_logical_callable());
        assert!(CallableDeclarationOwner::Proof.permits_proof_statement_call());

        assert!(!CallableDeclarationOwner::View.is_runtime_callable());
        assert!(!CallableDeclarationOwner::View.is_logical_callable());
        assert!(!CallableDeclarationOwner::View.permits_proof_statement_call());
        assert_eq!(CallableDeclarationOwner::View.as_str(), "view");

        assert!(!CallableDeclarationOwner::Flow.is_runtime_callable());
        assert!(!CallableDeclarationOwner::Flow.is_logical_callable());
        assert!(!CallableDeclarationOwner::Flow.permits_proof_statement_call());
        assert!(!CallableDeclarationOwner::Flow.is_method());
        assert_eq!(CallableDeclarationOwner::Flow.as_str(), "flow");
        assert_eq!(CallableDeclarationOwner::Flow.digest_tag(), 8);
    }

    #[test]
    fn world_and_revision_are_validated_typed_boundaries() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/main").unwrap(),
            SourceName::path("main.arcw"),
            "flow main {}",
        )
        .unwrap();
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("game").unwrap(),
            document.identity().id().clone(),
            "default",
        )
        .unwrap();
        let revision = ProjectSymbolRevision::try_for_documents([document.identity()]).unwrap();
        assert_eq!(world.profile(), "default");
        assert_ne!(revision.as_source_set().as_bytes(), &[0; 32]);
    }
}
