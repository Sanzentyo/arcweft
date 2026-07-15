use core::fmt;
use std::collections::BTreeMap;

use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathError},
    symbol_path::{SymbolPath, SymbolPathError},
};
use arcweft_source::{
    SourceDocumentId, SourceDocumentIdentity, SourceSetRevision, SourceSetRevisionError, SourceSpan,
};
use thiserror::Error;

use crate::model::HirFunction;

use super::qualified_name;

/// Canonical package component of a source declaration identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallablePackageId(String);

/// Identity of an original callable declaration before imports or re-exports.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    owner: CallableDeclarationOwner,
    name: String,
}

/// Source declaration family that owns a callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDeclarationOwner {
    Function,
    Predicate,
    Proof,
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
    name: String,
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
}

/// Unified project declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectSymbol {
    Callable(CallableSymbol),
    External(ExternalSymbol),
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
            name,
        })
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn qualified_name(&self) -> String {
        qualified_name(&self.module, self.name())
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
        name: impl Into<String>,
        visibility: Option<Visibility>,
        source: SourceSpan,
        authored_alias: bool,
    ) -> Result<Self, SymbolPathError> {
        let name = name.into();
        SymbolPath::try_new(
            arcweft_lang_syntax::ast::module_path::ModulePathRoot::ImplicitCrate,
            Vec::new(),
            name.clone(),
        )?;
        Ok(Self {
            module,
            name,
            visibility,
            source,
            authored_alias,
        })
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub fn name(&self) -> &str {
        &self.name
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
