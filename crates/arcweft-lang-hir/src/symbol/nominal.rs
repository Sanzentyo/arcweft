//! Source-backed project nominal declarations.
//!
//! This module owns immutable identities and declaration records for authored
//! structs, enums, and type aliases. Project-symbol linking is responsible for
//! constructing and publishing them; semantic consumers only observe the
//! records through the read-only accessors below.

use std::collections::BTreeSet;

use arcweft_lang_syntax::{
    ast::{
        common::{TextRange, Visibility},
        module_path::{CanonicalModulePath, ModulePathError, ModuleSegment},
    },
    types::{AuthoredTypeRef, TypeRefSourceMap, TypeRefSourceMapError},
};
use arcweft_source::{SourceDocument, SourceDocumentIdentity, SourceRange, SourceSpan};

use super::{ProjectSymbolLimitKind, ProjectSymbolRevision, ProjectSymbolWorldId, qualified_name};

/// Authored nominal declaration family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectNominalDeclarationKind {
    Struct,
    Enum,
    TypeAlias,
}

/// Stable identity of one nominal declaration in an accepted project world.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectNominalDeclarationId {
    pub(super) world: ProjectSymbolWorldId,
    pub(super) revision: ProjectSymbolRevision,
    pub(super) module: CanonicalModulePath,
    pub(super) kind: ProjectNominalDeclarationKind,
    pub(super) owner_path: Box<[ModuleSegment]>,
    pub(super) name: ModuleSegment,
}

/// A parsed type reference bound to the exact source-document revision that
/// owns its declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedTypeRef {
    authored: AuthoredTypeRef,
    spans: TypeRefSourceMap<SourceSpan>,
}

/// Source ranges for one nominal type parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalTypeParameterSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
}

/// One source-ordered generic type parameter on a nominal declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalTypeParameter {
    pub(super) ordinal: u16,
    pub(super) name: ModuleSegment,
    pub(super) bounds: Box<[SourceBackedTypeRef]>,
    pub(super) source: ProjectNominalTypeParameterSource,
}

/// A source-backed `where` predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedWherePredicate {
    pub(super) subject: SourceBackedTypeRef,
    pub(super) bounds: Box<[SourceBackedTypeRef]>,
    pub(super) whole: SourceSpan,
}

/// Source ranges for one struct field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalFieldSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
}

/// One typed field in a project struct declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalField {
    pub(super) name: ModuleSegment,
    pub(super) ty: SourceBackedTypeRef,
    pub(super) source: ProjectNominalFieldSource,
}

/// Source ranges for one enum variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalVariantSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
    pub(super) payload: Option<SourceSpan>,
}

/// One typed variant in a project enum declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalVariant {
    pub(super) name: ModuleSegment,
    pub(super) payload: Option<SourceBackedTypeRef>,
    pub(super) source: ProjectNominalVariantSource,
}

/// Family-specific body of an authored nominal declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectNominalBody {
    Struct {
        fields: Box<[ProjectNominalField]>,
    },
    Enum {
        variants: Box<[ProjectNominalVariant]>,
    },
    TypeAlias {
        target: SourceBackedTypeRef,
    },
}

/// Source ranges shared by every nominal declaration family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalDeclarationSource {
    pub(super) whole: SourceSpan,
    pub(super) name: SourceSpan,
    pub(super) generics: Option<SourceSpan>,
}

/// Immutable source-backed project nominal declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalDeclaration {
    pub(super) id: ProjectNominalDeclarationId,
    pub(super) visibility: Option<Visibility>,
    pub(super) type_parameters: Box<[ProjectNominalTypeParameter]>,
    pub(super) where_predicates: Box<[SourceBackedWherePredicate]>,
    pub(super) body: ProjectNominalBody,
    pub(super) source: ProjectNominalDeclarationSource,
}

/// Invalid authored nominal declaration encountered during project linking.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectNominalDeclarationError {
    InvalidName {
        source: SourceSpan,
        reason: ModulePathError,
    },
    UnsupportedLifetimeParameter {
        source: SourceSpan,
    },
    DuplicateTypeParameter {
        name: ModuleSegment,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    SourceMapMismatch {
        source: SourceSpan,
        reason: Box<ProjectNominalSourceError>,
    },
    Limit {
        kind: ProjectSymbolLimitKind,
        observed: u64,
        maximum: u64,
        source: SourceSpan,
    },
}

/// Failure while binding parser-local type ranges to an accepted document.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectNominalSourceError {
    Structure(TypeRefSourceMapError),
    OutOfBounds {
        range: TextRange,
        source_len: u32,
    },
    NotUtf8Boundary {
        byte: u32,
    },
    WrongDocument {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}

impl ProjectNominalDeclarationId {
    /// Project-symbol world that owns the declaration.
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    /// Exact accepted source-set revision.
    pub const fn revision(&self) -> ProjectSymbolRevision {
        self.revision
    }

    /// Canonical module that originally declares the nominal.
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    /// Declaration family, which participates in identity.
    pub const fn kind(&self) -> ProjectNominalDeclarationKind {
        self.kind
    }

    /// Future owner path, empty for every currently authored top-level nominal.
    pub fn owner_path(&self) -> &[ModuleSegment] {
        &self.owner_path
    }

    /// Declaration-local validated name.
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    /// Qualified display spelling; never an identity parser input.
    pub fn qualified_name(&self) -> String {
        if self.owner_path.is_empty() {
            return qualified_name(&self.module, self.name.as_str());
        }

        let owner_len = self
            .owner_path
            .iter()
            .map(|segment| segment.as_str().len() + 1)
            .sum::<usize>();
        let mut local = String::with_capacity(owner_len + self.name.as_str().len());
        for segment in &self.owner_path {
            local.push_str(segment.as_str());
            local.push('.');
        }
        local.push_str(self.name.as_str());
        qualified_name(&self.module, &local)
    }
}

impl SourceBackedTypeRef {
    /// Binds every parser-owned local range through the exact accepted module
    /// document. The expected identity prevents a caller from substituting a
    /// different document or revision while retaining the same local ranges.
    ///
    /// # Panics
    ///
    /// Panics only if a caller bypasses the project source-size admission
    /// limit and supplies a document longer than `u32::MAX` bytes, or if the
    /// syntax/source range invariants were already violated internally.
    #[allow(
        clippy::result_large_err,
        reason = "the final source-binding error preserves both complete document identities"
    )]
    pub fn try_bind(
        authored: AuthoredTypeRef,
        document: &SourceDocument,
        expected: &SourceDocumentIdentity,
    ) -> Result<Self, ProjectNominalSourceError> {
        if document.identity() != expected {
            return Err(ProjectNominalSourceError::WrongDocument {
                expected: expected.clone(),
                actual: document.identity().clone(),
            });
        }

        let source_len = u32::try_from(document.text().len())
            .expect("accepted Arcweft source documents fit the u32 source contract");
        let spans = authored.source().try_map(|range| {
            if range.start() > range.end() || range.end() > document.text().len() {
                return Err(ProjectNominalSourceError::OutOfBounds {
                    range: *range,
                    source_len,
                });
            }
            for byte in [range.start(), range.end()] {
                if !document.text().is_char_boundary(byte) {
                    return Err(ProjectNominalSourceError::NotUtf8Boundary {
                        byte: u32::try_from(byte)
                            .expect("in-bounds accepted source offsets fit u32"),
                    });
                }
            }
            Ok(document
                .span(SourceRange::new(range.start(), range.end()))
                .expect("prevalidated source range binds to its source document"))
        })?;

        validate_bound_node_paths(authored.source(), &spans)?;
        Ok(Self { authored, spans })
    }

    /// Parser-owned typed structure and local ranges.
    pub const fn authored(&self) -> &AuthoredTypeRef {
        &self.authored
    }

    /// Exact document-bound span for every structural type node.
    pub const fn spans(&self) -> &TypeRefSourceMap<SourceSpan> {
        &self.spans
    }
}

#[allow(
    clippy::result_large_err,
    reason = "the final source-binding error preserves both complete document identities"
)]
fn validate_bound_node_paths(
    local: &TypeRefSourceMap<TextRange>,
    bound: &TypeRefSourceMap<SourceSpan>,
) -> Result<(), ProjectNominalSourceError> {
    let local_paths = local
        .nodes()
        .iter()
        .map(|(path, _)| path)
        .collect::<BTreeSet<_>>();
    if !local_paths.contains(&arcweft_lang_syntax::types::TypeRefNodePath::root()) {
        return Err(ProjectNominalSourceError::Structure(
            TypeRefSourceMapError::MissingRoot,
        ));
    }
    if local_paths.len() != local.nodes().len() {
        let duplicate = local
            .nodes()
            .windows(2)
            .find_map(|nodes| (nodes[0].0 == nodes[1].0).then(|| nodes[0].0.clone()))
            .expect("a repeated path exists when unique cardinality is smaller");
        return Err(ProjectNominalSourceError::Structure(
            TypeRefSourceMapError::DuplicateNode(duplicate),
        ));
    }

    let bound_paths = bound
        .nodes()
        .iter()
        .map(|(path, _)| path)
        .collect::<BTreeSet<_>>();
    if bound_paths.len() != bound.nodes().len() {
        let duplicate = bound
            .nodes()
            .windows(2)
            .find_map(|nodes| (nodes[0].0 == nodes[1].0).then(|| nodes[0].0.clone()))
            .expect("a repeated path exists when unique cardinality is smaller");
        return Err(ProjectNominalSourceError::Structure(
            TypeRefSourceMapError::DuplicateNode(duplicate),
        ));
    }
    if let Some(missing) = local_paths.difference(&bound_paths).next() {
        return Err(ProjectNominalSourceError::Structure(
            TypeRefSourceMapError::MissingNode((*missing).clone()),
        ));
    }
    if let Some(extra) = bound_paths.difference(&local_paths).next() {
        return Err(ProjectNominalSourceError::Structure(
            TypeRefSourceMapError::ExtraNode((*extra).clone()),
        ));
    }
    Ok(())
}

impl ProjectNominalTypeParameterSource {
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }

    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }
}

impl ProjectNominalTypeParameter {
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub fn bounds(&self) -> &[SourceBackedTypeRef] {
        &self.bounds
    }

    pub const fn source(&self) -> &ProjectNominalTypeParameterSource {
        &self.source
    }
}

impl SourceBackedWherePredicate {
    pub const fn subject(&self) -> &SourceBackedTypeRef {
        &self.subject
    }

    pub fn bounds(&self) -> &[SourceBackedTypeRef] {
        &self.bounds
    }

    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
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

impl ProjectNominalField {
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub const fn ty(&self) -> &SourceBackedTypeRef {
        &self.ty
    }

    pub const fn source(&self) -> &ProjectNominalFieldSource {
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

impl ProjectNominalVariant {
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub const fn payload(&self) -> Option<&SourceBackedTypeRef> {
        self.payload.as_ref()
    }

    pub const fn source(&self) -> &ProjectNominalVariantSource {
        &self.source
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

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn type_parameters(&self) -> &[ProjectNominalTypeParameter] {
        &self.type_parameters
    }

    pub fn where_predicates(&self) -> &[SourceBackedWherePredicate] {
        &self.where_predicates
    }

    pub const fn body(&self) -> &ProjectNominalBody {
        &self.body
    }

    pub const fn source(&self) -> &ProjectNominalDeclarationSource {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use arcweft_lang_syntax::types::{TypeRefNodeStep, parse_type_ref};
    use arcweft_source::{SourceDocumentId, SourceName};

    use super::*;

    fn document(id: &str, text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("valid test document ID"),
            SourceName::Memory,
            text,
        )
        .expect("test source document")
    }

    #[test]
    fn source_binding_preserves_one_to_one_paths_and_exact_utf8_spans() {
        let text = "Result<\n  Missing,\n  名前.Type\n>";
        let authored = parse_type_ref(text).expect("nested type parses");
        let document = document("arcw:/nominal/type", text);
        let bound = SourceBackedTypeRef::try_bind(authored, &document, document.identity())
            .expect("type source binds");

        let local_paths = bound
            .authored()
            .source()
            .nodes()
            .iter()
            .map(|(path, _)| path.steps())
            .collect::<Vec<_>>();
        let bound_paths = bound
            .spans()
            .nodes()
            .iter()
            .map(|(path, _)| path.steps())
            .collect::<Vec<_>>();
        assert_eq!(local_paths, bound_paths);

        let utf8 = text.find("名前.Type").expect("UTF-8 type path");
        let (_, source) = bound
            .spans()
            .nodes()
            .iter()
            .find(|(path, _)| path.steps() == [TypeRefNodeStep::GenericArgument(1)])
            .expect("second generic argument source");
        assert_eq!(
            source.whole().range(),
            SourceRange::new(utf8, utf8 + "名前.Type".len())
        );
        assert_eq!(source.whole().source(), document.identity());
    }

    #[test]
    fn source_binding_rejects_out_of_bounds_ranges_without_clamping() {
        let authored = parse_type_ref("Missing").expect("type parses");
        let document = document("arcw:/nominal/short", "T");

        assert_eq!(
            SourceBackedTypeRef::try_bind(authored, &document, document.identity()),
            Err(ProjectNominalSourceError::OutOfBounds {
                range: TextRange::new(0, "Missing".len()),
                source_len: 1,
            })
        );
    }

    #[test]
    fn source_binding_rejects_non_utf8_boundary_ranges() {
        let authored = parse_type_ref("T").expect("type parses");
        let document = document("arcw:/nominal/utf8", "é");

        assert_eq!(
            SourceBackedTypeRef::try_bind(authored, &document, document.identity()),
            Err(ProjectNominalSourceError::NotUtf8Boundary { byte: 1 })
        );
    }

    #[test]
    fn source_binding_rejects_a_substituted_document_identity() {
        let authored = parse_type_ref("T").expect("type parses");
        let actual = document("arcw:/nominal/actual", "T");
        let expected = document("arcw:/nominal/expected", "T");

        assert_eq!(
            SourceBackedTypeRef::try_bind(authored, &actual, expected.identity()),
            Err(ProjectNominalSourceError::WrongDocument {
                expected: expected.identity().clone(),
                actual: actual.identity().clone(),
            })
        );
    }

    #[test]
    fn nominal_identity_keeps_owner_kind_world_and_revision_components() {
        use super::super::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};

        let document = document("arcw:/nominal/identity", "struct Thing {}\n");
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("game").expect("package"),
            document.identity().id().clone(),
            "test",
        )
        .expect("world");
        let revision = ProjectSymbolRevision::try_for_documents([document.identity()])
            .expect("source-set revision");
        let module = CanonicalModulePath::from_segments([
            ModuleSegment::new("model").expect("module segment")
        ]);
        let owner = ModuleSegment::new("Nested").expect("owner segment");
        let name = ModuleSegment::new("Thing").expect("name segment");
        let base = ProjectNominalDeclarationId {
            world,
            revision,
            module,
            kind: ProjectNominalDeclarationKind::Struct,
            owner_path: vec![owner].into_boxed_slice(),
            name,
        };
        let mut changed = base.clone();
        changed.kind = ProjectNominalDeclarationKind::Enum;

        assert_ne!(base, changed);
        assert_eq!(base.qualified_name(), "model.Nested.Thing");
        assert_eq!(base.kind(), ProjectNominalDeclarationKind::Struct);
        assert_eq!(base.owner_path()[0].as_str(), "Nested");
    }
}
