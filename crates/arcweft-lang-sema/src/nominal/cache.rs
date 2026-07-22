//! Exact-key cache for accepted authored type-reference resolution.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModulePathRoot},
    types::{AuthoredTypeRef, TypePath, TypeRef, TypeRefNodePath},
};
use arcweft_source::SourceSpan;

use crate::env::nominal::AcceptedNominalCatalogDigest;

use super::{
    GenericTypeScopeFingerprint, NominalResolutionLimits, SelfTypeScopeFingerprint,
    TypeResolutionInput, TypeResolutionInputError, TypeResolutionReport, TypeResolutionWorld,
};

/// Version of the structural resolver/cache semantics represented by this key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalResolverSchemaVersion(u16);

impl NominalResolverSchemaVersion {
    /// Initial direct-final project nominal resolver schema.
    pub const CURRENT: Self = Self(1);

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Deterministic digest of the typed structure, excluding source coordinates.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredTypeRefStructuralDigest([u8; 32]);

impl AuthoredTypeRefStructuralDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Complete accepted-world key for one checked authored type reference.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTypeReferenceCacheKey {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    module: CanonicalModulePath,
    root: SourceSpan,
    authored: AuthoredTypeRefStructuralDigest,
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
        } = input.world()
        else {
            return None;
        };
        let module = input.current_module()?;
        let source_backed = input.authored().source_backed()?;
        let root = source_backed
            .spans()
            .source_at(&TypeRefNodePath::root())
            .expect("accepted authored type source maps contain their root")
            .whole()
            .clone();
        Some(Self {
            world: symbols.world().clone(),
            revision: *symbols.revision(),
            module: module.clone(),
            root,
            authored: structural_digest(source_backed.authored()),
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

    pub const fn root(&self) -> &SourceSpan {
        &self.root
    }

    pub const fn authored(&self) -> AuthoredTypeRefStructuralDigest {
        self.authored
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
    ///
    /// Detached inputs deliberately bypass this cache because no detached HIR
    /// arena identity is part of their input contract.
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

fn structural_digest(authored: &AuthoredTypeRef) -> AuthoredTypeRefStructuralDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft-authored-type-ref-structure-v1\0");
    hash_type_ref(authored.value(), &mut hasher);
    AuthoredTypeRefStructuralDigest(*hasher.finalize().as_bytes())
}

enum TypeHashTask<'a> {
    Type(&'a TypeRef),
    Str(&'a str),
    U64(u64),
    Byte(u8),
}

fn hash_type_ref(ty: &TypeRef, hasher: &mut blake3::Hasher) {
    let mut pending = vec![TypeHashTask::Type(ty)];
    while let Some(task) = pending.pop() {
        match task {
            TypeHashTask::Str(value) => hash_str(value, hasher),
            TypeHashTask::U64(value) => hash_u64(value, hasher),
            TypeHashTask::Byte(value) => {
                hasher.update(&[value]);
            }
            TypeHashTask::Type(ty) => schedule_type_hash(ty, hasher, &mut pending),
        }
    }
}

fn schedule_type_hash<'a>(
    ty: &'a TypeRef,
    hasher: &mut blake3::Hasher,
    pending: &mut Vec<TypeHashTask<'a>>,
) {
    match ty {
        TypeRef::Never => {
            hasher.update(&[0]);
        }
        TypeRef::ConstInt(value) => {
            hasher.update(&[1]);
            hash_u64(
                u64::try_from(*value).expect("accepted type constants fit u64"),
                hasher,
            );
        }
        TypeRef::Path(path) => {
            hasher.update(&[2]);
            hash_type_path(path, hasher);
        }
        TypeRef::Tuple(items) => schedule_type_sequence(3, items, hasher, pending),
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => schedule_function_hash(params, return_type, effects.as_ref(), hasher, pending),
        TypeRef::Choice(alternatives) => {
            schedule_type_sequence(5, alternatives, hasher, pending);
        }
        TypeRef::Generic { base, args } => {
            hasher.update(&[6]);
            hash_type_path(base, hasher);
            hash_u64(args.len() as u64, hasher);
            pending.extend(args.iter().rev().map(TypeHashTask::Type));
        }
        TypeRef::TraitBound(bound) => schedule_trait_hash(bound, hasher, pending),
        TypeRef::Projection { subject, assoc } => {
            hasher.update(&[8]);
            pending.push(TypeHashTask::Str(assoc.as_str()));
            pending.push(TypeHashTask::Type(subject));
        }
        TypeRef::Reference(reference) => {
            hasher.update(&[9]);
            hasher.update(&[u8::from(reference.kind().is_mutable())]);
            match reference.region().name() {
                Some(region) => {
                    hasher.update(&[1]);
                    hash_str(region.name(), hasher);
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            pending.push(TypeHashTask::Type(reference.referent()));
        }
        TypeRef::Slice(item) => {
            hasher.update(&[10]);
            pending.push(TypeHashTask::Type(item));
        }
        TypeRef::Recovery(recovery) => {
            hasher.update(&[11]);
            hasher.update(&recovery.index().to_le_bytes());
        }
    }
}

fn schedule_type_sequence<'a>(
    tag: u8,
    items: &'a [TypeRef],
    hasher: &mut blake3::Hasher,
    pending: &mut Vec<TypeHashTask<'a>>,
) {
    hasher.update(&[tag]);
    hash_u64(items.len() as u64, hasher);
    pending.extend(items.iter().rev().map(TypeHashTask::Type));
}

fn schedule_function_hash<'a>(
    params: &'a [TypeRef],
    return_type: &'a TypeRef,
    effects: Option<&'a arcweft_lang_syntax::types::TypeEffectRow>,
    hasher: &mut blake3::Hasher,
    pending: &mut Vec<TypeHashTask<'a>>,
) {
    hasher.update(&[4]);
    hash_u64(params.len() as u64, hasher);
    if let Some(effects) = effects {
        pending.extend(
            effects
                .effects()
                .iter()
                .rev()
                .map(|effect| TypeHashTask::Str(effect.as_str())),
        );
        pending.push(TypeHashTask::U64(effects.effects().len() as u64));
        pending.push(TypeHashTask::Byte(1));
    } else {
        pending.push(TypeHashTask::Byte(0));
    }
    pending.push(TypeHashTask::Type(return_type));
    pending.extend(params.iter().rev().map(TypeHashTask::Type));
}

fn schedule_trait_hash<'a>(
    bound: &'a arcweft_lang_syntax::types::TraitBound,
    hasher: &mut blake3::Hasher,
    pending: &mut Vec<TypeHashTask<'a>>,
) {
    hasher.update(&[7]);
    hash_type_path(bound.path(), hasher);
    hash_u64(bound.args().len() as u64, hasher);
    for binding in bound.associated().iter().rev() {
        pending.push(TypeHashTask::Type(binding.value()));
        pending.push(TypeHashTask::Str(binding.name().as_str()));
    }
    pending.push(TypeHashTask::U64(bound.associated().len() as u64));
    pending.extend(bound.args().iter().rev().map(TypeHashTask::Type));
}

fn hash_type_path(path: &TypePath, hasher: &mut blake3::Hasher) {
    match path.root() {
        ModulePathRoot::ImplicitCrate => {
            hasher.update(&[0]);
        }
        ModulePathRoot::Crate => {
            hasher.update(&[1]);
        }
        ModulePathRoot::SelfModule => {
            hasher.update(&[2]);
        }
        ModulePathRoot::Super(levels) => {
            hasher.update(&[3]);
            hash_u64(levels as u64, hasher);
        }
    }
    hash_u64(path.segments().len() as u64, hasher);
    for segment in path.segments() {
        hash_str(segment.as_str(), hasher);
    }
}

fn hash_str(value: &str, hasher: &mut blake3::Hasher) {
    hash_u64(value.len() as u64, hasher);
    hasher.update(value.as_bytes());
}

fn hash_u64(value: u64, hasher: &mut blake3::Hasher) {
    hasher.update(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use arcweft_lang_hir::symbol::nominal::{ProjectNominalBody, SourceBackedTypeRef};
    use arcweft_lang_syntax::{
        ast::{
            common::TextRange,
            module_path::{CanonicalModulePath, ModuleSegment},
        },
        types::{TypeRef, parse_type_ref},
    };

    use crate::{
        env::{
            TypeCheckEnv,
            nominal::{
                AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
                AcceptedNominalRecord, AcceptedNominalSemantics,
            },
        },
        nominal::{GenericTypeBinding, GenericTypeScope, SelfTypeScope, TypeSourceEvidence},
        registration::RegisteredSemanticWorld,
        test_support::character_project::{
            one_character_facts, one_character_facts_with_documents, project_modules, register,
            root_project_source, sample_manifest,
        },
        types::{DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId, TypeKind},
    };

    use super::*;

    fn registered(source: &str, profile: &str, base: TypeCheckEnv) -> RegisteredSemanticWorld {
        let (document, project, world) = root_project_source(profile, source);
        let facts = one_character_facts(&document, world, &sample_manifest("layers/body.png"));
        register(&project, &facts, base, None).expect("cache fixture registers")
    }

    fn field_type<'a>(
        world: &'a RegisteredSemanticWorld,
        declaration_name: &str,
    ) -> &'a SourceBackedTypeRef {
        let declaration = world
            .symbols()
            .nominal_symbols()
            .find(|declaration| declaration.id().name().as_str() == declaration_name)
            .expect("fixture declaration exists");
        let ProjectNominalBody::Struct { fields } = declaration.body() else {
            panic!("fixture declaration is a struct")
        };
        fields.first().expect("fixture field exists").ty()
    }

    fn accepted_input<'a>(
        authored: &'a SourceBackedTypeRef,
        module: &'a CanonicalModulePath,
        world: &'a RegisteredSemanticWorld,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeResolutionInput<'a> {
        TypeResolutionInput::accepted(
            authored,
            module,
            world.symbols(),
            world.environment().nominal_world(),
            generics,
            self_scope,
            NominalResolutionLimits::PRODUCTION,
        )
        .expect("cache fixture input is accepted")
    }

    fn generic_scope(owner: u64) -> GenericTypeScope {
        GenericTypeScope::try_new([GenericTypeBinding::new(
            GenericTypeParameterId::new(
                GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(owner)),
                0,
            ),
            ModuleSegment::new("T").expect("generic name"),
            TypeSourceEvidence::detached(TextRange::new(0, 1)),
        )])
        .expect("generic scope")
    }

    fn type_path(source: &str) -> TypePath {
        let authored = parse_type_ref(source).expect("type path");
        let TypeRef::Path(path) = authored.value() else {
            panic!("fixture is a direct type path")
        };
        path.clone()
    }

    #[test]
    fn structural_digest_ignores_trivia_but_retains_semantics() {
        let compact = parse_type_ref("Result<Vec<T>, E>").expect("compact type");
        let spaced = parse_type_ref("Result< Vec<T> , E >").expect("spaced type");
        let different = parse_type_ref("Result<Vec<U>, E>").expect("different type");

        assert_eq!(structural_digest(&compact), structural_digest(&spaced));
        assert_ne!(structural_digest(&compact), structural_digest(&different));
    }

    #[test]
    fn resolver_schema_version_is_explicit() {
        assert_eq!(NominalResolverSchemaVersion::CURRENT.value(), 1);
    }

    #[test]
    fn accepted_cache_key_supports_the_production_recursive_depth() {
        let mut field_source = "i32".to_owned();
        for _ in 0..255 {
            field_source = format!("Option<{field_source}>");
        }
        let world = registered(
            &format!("pub struct Deep {{ value: {field_source} }}\n"),
            "cache-production-depth",
            TypeCheckEnv::standard(),
        );
        let authored = field_type(&world, "Deep");
        let module = CanonicalModulePath::crate_root();
        let generics = GenericTypeScope::empty();
        let mut cache = CheckedTypeReferenceCache::default();

        let report = cache
            .resolve(&accepted_input(
                authored,
                &module,
                &world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("the accepted depth is hashable and resolvable");

        assert!(report.diagnostics().is_empty());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn exact_accepted_key_reuses_the_complete_report() {
        let world = registered(
            "pub struct Boxed { value: Vec<i32> }\n",
            "cache-hit",
            TypeCheckEnv::standard(),
        );
        let authored = field_type(&world, "Boxed");
        let module = CanonicalModulePath::crate_root();
        let generics = GenericTypeScope::empty();
        let mut cache = CheckedTypeReferenceCache::default();

        let first = cache
            .resolve(&accepted_input(
                authored,
                &module,
                &world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("first resolution");
        let second = cache
            .resolve(&accepted_input(
                authored,
                &module,
                &world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("cached resolution");

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 1);
        assert_eq!(first.as_ref(), second.as_ref());
    }

    #[test]
    fn generic_and_self_fingerprints_never_cross_reuse() {
        let generic_world = registered(
            "pub struct Boxed<T> { value: T }\n",
            "cache-generic",
            TypeCheckEnv::standard(),
        );
        let generic_authored = field_type(&generic_world, "Boxed");
        let module = CanonicalModulePath::crate_root();
        let first_scope = generic_scope(1);
        let second_scope = generic_scope(2);
        let mut cache = CheckedTypeReferenceCache::default();
        cache
            .resolve(&accepted_input(
                generic_authored,
                &module,
                &generic_world,
                &first_scope,
                SelfTypeScope::Absent,
            ))
            .expect("first generic resolution");
        cache
            .resolve(&accepted_input(
                generic_authored,
                &module,
                &generic_world,
                &second_scope,
                SelfTypeScope::Absent,
            ))
            .expect("second generic resolution");

        let self_world = registered(
            "pub struct Recursive { value: Self }\n",
            "cache-self",
            TypeCheckEnv::standard(),
        );
        let self_authored = field_type(&self_world, "Recursive");
        let empty = GenericTypeScope::empty();
        cache
            .resolve(&accepted_input(
                self_authored,
                &module,
                &self_world,
                &empty,
                SelfTypeScope::Known(TypeKind::Bool),
            ))
            .expect("first Self resolution");
        cache
            .resolve(&accepted_input(
                self_authored,
                &module,
                &self_world,
                &empty,
                SelfTypeScope::Known(TypeKind::String),
            ))
            .expect("second Self resolution");

        assert_eq!(cache.len(), 4);
        assert_eq!(cache.hits(), 0);
        assert_eq!(cache.misses(), 4);
    }

    #[test]
    fn module_is_an_independent_cache_key_component() {
        let sources = [
            ("", "pub struct RootBox { value: Local }\n"),
            ("child", "pub struct ChildBox { value: Local }\n"),
        ];
        let (documents, project, world_id) = project_modules("cache-module", &sources);
        let facts = one_character_facts_with_documents(
            &documents[0],
            documents.clone(),
            world_id,
            &sample_manifest("layers/body.png"),
        );
        let world = register(&project, &facts, TypeCheckEnv::standard(), None)
            .expect("module fixture registers");
        let root = field_type(&world, "RootBox");
        let child = field_type(&world, "ChildBox");
        let root_module = CanonicalModulePath::crate_root();
        let child_module = root_module.join(ModuleSegment::new("child").expect("child module"));
        let generics = GenericTypeScope::empty();
        let mut cache = CheckedTypeReferenceCache::default();

        cache
            .resolve(&accepted_input(
                root,
                &root_module,
                &world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("root resolution");
        cache
            .resolve(&accepted_input(
                child,
                &child_module,
                &world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("child resolution");

        let modules = cache
            .entries()
            .map(|(key, _)| key.module().clone())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(modules.len(), 2);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn catalog_digest_and_project_revision_never_cross_reuse() {
        let source = "pub struct Boxed { value: i32 }\n";
        let (document, project, world_id) = root_project_source("cache-env", source);
        let facts = one_character_facts(&document, world_id, &sample_manifest("layers/body.png"));
        let base = TypeCheckEnv::standard();
        let extra = AcceptedNominalRecord::try_new(
            AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, type_path("CacheExtra")),
            0,
            AcceptedNominalSemantics::Exact(TypeKind::String),
            AcceptedNominalOrigin::Domain,
            None,
        )
        .expect("extra nominal record");
        let changed_base = base
            .clone()
            .try_with_nominal_record(extra)
            .expect("extra catalog record");
        let first_world = register(&project, &facts, base, None).expect("first environment");
        let changed_world =
            register(&project, &facts, changed_base, None).expect("changed environment");
        let first_authored = field_type(&first_world, "Boxed");
        let changed_authored = field_type(&changed_world, "Boxed");
        let module = CanonicalModulePath::crate_root();
        let generics = GenericTypeScope::empty();
        let mut cache = CheckedTypeReferenceCache::default();

        cache
            .resolve(&accepted_input(
                first_authored,
                &module,
                &first_world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("first catalog");
        cache
            .resolve(&accepted_input(
                changed_authored,
                &module,
                &changed_world,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("changed catalog");
        assert_eq!(cache.len(), 2);

        let first_revision = registered(source, "cache-revision", TypeCheckEnv::standard());
        let second_revision = registered(
            "pub struct Boxed { value: i32 }\n// new revision\n",
            "cache-revision",
            TypeCheckEnv::standard(),
        );
        let first_revision_authored = field_type(&first_revision, "Boxed");
        let second_revision_authored = field_type(&second_revision, "Boxed");
        cache
            .resolve(&accepted_input(
                first_revision_authored,
                &module,
                &first_revision,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("first revision");
        cache
            .resolve(&accepted_input(
                second_revision_authored,
                &module,
                &second_revision,
                &generics,
                SelfTypeScope::Absent,
            ))
            .expect("second revision");

        let revisions = cache
            .entries()
            .filter(|(key, _)| key.world().profile() == "cache-revision")
            .map(|(key, _)| key.revision())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(revisions.len(), 2);
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.hits(), 0);
    }
}
