use core::fmt;
use std::collections::BTreeMap;

use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathError, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, SymbolPath},
};
use arcweft_source::{
    SourceDocumentId, SourceDocumentIdentity, SourceSetRevision, SourceSetRevisionError, SourceSpan,
};
use thiserror::Error;

use crate::model::HirFunction;

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

/// Source declaration family that owns a callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDeclarationOwner {
    Function,
    ExternCapability,
    View,
    Predicate,
    Proof,
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
        }
    }

    /// Whether declarations of this family can become runtime call targets.
    pub const fn is_runtime_callable(self) -> bool {
        matches!(self, Self::Function | Self::ExternCapability)
    }

    /// Whether declarations of this family denote logical Boolean callables.
    pub const fn is_logical_callable(self) -> bool {
        matches!(self, Self::Predicate)
    }

    /// Whether declarations of this family can be invoked as proof statements.
    pub const fn permits_proof_statement_call(self) -> bool {
        matches!(self, Self::Proof)
    }
}

/// One callable declaration indexed independently from every source alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSymbol {
    pub(super) declaration: CallableDeclarationId,
    pub(super) visibility: Option<Visibility>,
    pub(super) fx: bool,
    pub(super) source: SourceSpan,
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

/// Unified declaration identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectDeclarationId {
    Callable(CallableDeclarationId),
    External(ExternalDeclarationId),
    Nominal(super::nominal::ProjectNominalDeclarationId),
}

/// Unified project declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectSymbol {
    Callable(CallableSymbol),
    External(ExternalSymbol),
    Nominal(Box<ProjectNominalDeclaration>),
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

    pub fn for_function(
        package: &CallablePackageId,
        function: &HirFunction,
    ) -> Result<Self, CallableDeclarationIdError> {
        let module = function.module_path().cloned().ok_or_else(|| {
            CallableDeclarationIdError::MissingModule {
                name: function.name().to_owned(),
            }
        })?;
        Self::try_new(
            package.clone(),
            module,
            CallableDeclarationOwner::Function,
            function.name(),
        )
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

impl CallableSymbol {
    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn is_fx(&self) -> bool {
        self.fx
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
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
    }

    #[test]
    fn world_and_revision_are_validated_typed_boundaries() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/main").unwrap(),
            SourceName::path("main.arcw"),
            "flow @main main {}",
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
