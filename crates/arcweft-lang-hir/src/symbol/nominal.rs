//! Project nominal declarations projected directly from final arena HIR.
//!
//! The symbol table retains module-qualified `ItemId`/`TypeId` identities and
//! revision-bound source evidence. It does not retain detached syntax trees or
//! rebuild authored type records from source text.

use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathError, ModuleSegment},
};
use arcweft_source::SourceSpan;

use crate::identity::{ItemId, TypeId};

use super::{ProjectSymbolLimitKind, ProjectSymbolRevision, ProjectSymbolWorldId, qualified_name};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectNominalDeclarationKind {
    Struct,
    Enum,
    TypeAlias,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectNominalDeclarationId {
    pub(super) world: ProjectSymbolWorldId,
    pub(super) revision: ProjectSymbolRevision,
    pub(super) module: CanonicalModulePath,
    pub(super) kind: ProjectNominalDeclarationKind,
    pub(super) owner_path: Box<[ModuleSegment]>,
    pub(super) name: ModuleSegment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalTypeParameter {
    pub(super) ordinal: u16,
    pub(super) name: ModuleSegment,
    pub(super) bounds: Box<[TypeId]>,
    pub(super) source: ProjectNominalTypeParameterSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalTypeParameterSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalWherePredicate {
    pub(super) subject: TypeId,
    pub(super) bounds: Box<[TypeId]>,
    pub(super) whole: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalField {
    pub(super) name: ModuleSegment,
    pub(super) ty: TypeId,
    pub(super) source: ProjectNominalFieldSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalFieldSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalVariant {
    pub(super) name: ModuleSegment,
    pub(super) payload: Option<TypeId>,
    pub(super) source: ProjectNominalVariantSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalVariantSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
    pub(super) payload: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectNominalBody {
    Struct {
        fields: Box<[ProjectNominalField]>,
    },
    Enum {
        variants: Box<[ProjectNominalVariant]>,
    },
    TypeAlias {
        target: TypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalDeclarationSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
    pub(super) generics: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalDeclaration {
    pub(super) id: ProjectNominalDeclarationId,
    pub(super) owner: ItemId,
    pub(super) visibility: Option<Visibility>,
    pub(super) type_parameters: Box<[ProjectNominalTypeParameter]>,
    pub(super) where_predicates: Box<[ProjectNominalWherePredicate]>,
    pub(super) body: ProjectNominalBody,
    pub(super) source: ProjectNominalDeclarationSource,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectNominalDeclarationError {
    InvalidName {
        source: SourceSpan,
        reason: ModulePathError,
    },
    RecoveredName {
        source: SourceSpan,
    },
    UnsupportedLifetimeParameter {
        source: SourceSpan,
    },
    DuplicateTypeParameter {
        name: ModuleSegment,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    Limit {
        kind: ProjectSymbolLimitKind,
        observed: u64,
        maximum: u64,
        source: SourceSpan,
    },
}

impl ProjectNominalDeclarationId {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }
    pub const fn revision(&self) -> ProjectSymbolRevision {
        self.revision
    }
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }
    pub const fn kind(&self) -> ProjectNominalDeclarationKind {
        self.kind
    }
    pub fn owner_path(&self) -> &[ModuleSegment] {
        &self.owner_path
    }
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub fn qualified_name(&self) -> String {
        let local = self
            .owner_path
            .iter()
            .map(ModuleSegment::as_str)
            .chain(std::iter::once(self.name.as_str()))
            .collect::<Vec<_>>()
            .join(".");
        qualified_name(&self.module, &local)
    }
}

impl ProjectNominalTypeParameter {
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }
    pub fn bounds(&self) -> &[TypeId] {
        &self.bounds
    }
    pub const fn source(&self) -> &ProjectNominalTypeParameterSource {
        &self.source
    }
}

impl ProjectNominalTypeParameterSource {
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }
    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }
}

impl ProjectNominalWherePredicate {
    pub const fn subject(&self) -> TypeId {
        self.subject
    }
    pub fn bounds(&self) -> &[TypeId] {
        &self.bounds
    }
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }
}

impl ProjectNominalField {
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }
    pub const fn ty(&self) -> TypeId {
        self.ty
    }
    pub const fn source(&self) -> &ProjectNominalFieldSource {
        &self.source
    }
}

impl ProjectNominalFieldSource {
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }
    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }
}

impl ProjectNominalVariant {
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }
    pub const fn payload(&self) -> Option<TypeId> {
        self.payload
    }
    pub const fn source(&self) -> &ProjectNominalVariantSource {
        &self.source
    }
}

impl ProjectNominalVariantSource {
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }
    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }
    pub const fn payload(&self) -> Option<&SourceSpan> {
        self.payload.as_ref()
    }
}

impl ProjectNominalDeclarationSource {
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }
    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }
    pub const fn generics(&self) -> Option<&SourceSpan> {
        self.generics.as_ref()
    }
}

impl ProjectNominalDeclaration {
    pub const fn id(&self) -> &ProjectNominalDeclarationId {
        &self.id
    }
    pub const fn owner(&self) -> ItemId {
        self.owner
    }
    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }
    pub fn type_parameters(&self) -> &[ProjectNominalTypeParameter] {
        &self.type_parameters
    }
    pub fn where_predicates(&self) -> &[ProjectNominalWherePredicate] {
        &self.where_predicates
    }
    pub const fn body(&self) -> &ProjectNominalBody {
        &self.body
    }
    pub const fn source(&self) -> &ProjectNominalDeclarationSource {
        &self.source
    }
}
