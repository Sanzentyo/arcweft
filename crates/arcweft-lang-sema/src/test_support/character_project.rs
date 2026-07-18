use std::sync::Arc;

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
        registration::{
            CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
        },
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
    },
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    character_definition::{
        CharacterDefinitionRequestBudget, CharacterReferenceInput, CharacterReferenceInventory,
        CharacterReferenceInventoryError, collect_character_references,
    },
    checker::analyze_registered_project_types,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ExternalRegistrationFact,
        ProjectRegistrationFacts, RegisteredExternalOwner, RegisteredSemanticWorld,
        RegisteredTypeCheckEnv,
    },
    types::{EntityKind, TypeKind},
};

pub(crate) const PACKAGE: &str = "registration-tests";

pub(crate) fn sample_manifest_for(owner: &str, asset: &str) -> CharacterManifest {
    let part = CharacterPartId::try_new("body").expect("part");
    let variant = CharacterVariantId::try_new("default").expect("variant");
    let look = CharacterLookId::try_new("normal").expect("look");
    CharacterManifest::new(
        CharacterId::try_new(owner).expect("character"),
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        look.clone(),
        vec![CharacterPart::new(
            part.clone(),
            0,
            vec![CharacterVariant::new(
                variant.clone(),
                CharacterAssetPath::try_new(asset).expect("asset"),
                CharacterRect::new(0, 0, 64, 128),
                u8::MAX,
                CharacterBlendMode::Normal,
                false,
            )],
        )],
        vec![CharacterLook::new(
            look,
            vec![CharacterPartSelection::new(part, variant)],
        )],
        None,
    )
    .expect("sample manifest")
}

pub(crate) fn sample_manifest(asset: &str) -> CharacterManifest {
    sample_manifest_for("character.akane", asset)
}

pub(crate) fn source_document(id: &str, source: impl Into<Arc<str>>) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("document id"),
            SourceName::path(id),
            source,
        )
        .expect("source document"),
    )
}

pub(crate) fn root_project(
    profile: &str,
) -> (Arc<SourceDocument>, HirProject, ProjectSymbolWorldId) {
    root_project_source(profile, "fn main() -> Unit { () }\n")
}

pub(crate) fn root_project_source(
    profile: &str,
    source: &str,
) -> (Arc<SourceDocument>, HirProject, ProjectSymbolWorldId) {
    let document = source_document("arcweft-project://registration-tests/src/main.arcw", source);
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered HIR");
    let project = HirProject::new(
        PACKAGE,
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("registration fixture module binding")],
    )
    .expect("HIR project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("package"),
        document.identity().id().clone(),
        profile,
    )
    .expect("world");
    (document, project, world)
}

pub(crate) fn project_modules(
    profile: &str,
    sources: &[(&str, &str)],
) -> (Vec<Arc<SourceDocument>>, HirProject, ProjectSymbolWorldId) {
    let mut documents = Vec::with_capacity(sources.len());
    let modules = sources
        .iter()
        .map(|(path, source)| {
            let file = if path.is_empty() {
                "main".to_owned()
            } else {
                path.replace('.', "/")
            };
            let document = source_document(
                &format!("arcweft-project://registration-tests/src/{file}.arcw"),
                *source,
            );
            let parsed = parse_source(*source);
            assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
            let hir =
                lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered module HIR");
            let path = path
                .split('.')
                .filter(|segment| !segment.is_empty())
                .fold(CanonicalModulePath::crate_root(), |module, segment| {
                    module.join(ModuleSegment::new(segment).expect("module segment"))
                });
            let module = HirProjectModule::try_new(path, document.identity().clone(), hir)
                .expect("registration fixture module binding");
            documents.push(document);
            module
        })
        .collect::<Vec<_>>();
    let project = HirProject::new(PACKAGE, modules).expect("HIR project");
    let root = documents.first().expect("root document");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("package"),
        root.identity().id().clone(),
        profile,
    )
    .expect("world");
    (documents, project, world)
}

pub(crate) fn backed_manifest(
    id: &str,
    manifest: &CharacterManifest,
) -> (Arc<SourceDocument>, SourceBackedCharacterManifest) {
    let source = manifest.to_json_pretty().expect("manifest JSON");
    let document = source_document(id, source);
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&document)
        .expect("source-backed manifest");
    (document, manifest)
}

pub(crate) fn declaration_span(
    manifest: &SourceBackedCharacterManifest,
) -> arcweft_source::SourceSpan {
    manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("character token")
        .value()
        .clone()
}

pub(crate) fn external_fact(
    canonical: &str,
    bindings: &[ProjectSymbolPath],
    target: RegisteredExternalOwner,
    declaration: arcweft_source::SourceSpan,
) -> ExternalRegistrationFact {
    let direct_bindings = bindings
        .iter()
        .map(|path| {
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                path.clone(),
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("direct bindings");
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), canonical)
            .expect("canonical path"),
        Some(Visibility::Public),
        declaration.clone(),
        direct_bindings,
    )
    .expect("external seed");
    ExternalRegistrationFact::new(seed, target, declaration)
}

pub(crate) fn project_path<const N: usize>(segments: [&str; N]) -> ProjectSymbolPath {
    ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        segments.map(|segment| {
            ProjectSymbolSegment::try_new(segment).expect("valid test project symbol segment")
        }),
    )
    .expect("test project symbol path is non-empty")
}

pub(crate) fn character_binding_paths(owner: &CharacterId) -> Vec<ProjectSymbolPath> {
    let compact_segments = owner
        .compact_segments()
        .map(|segment| {
            ProjectSymbolSegment::try_new(segment)
                .expect("character compact segments are valid project symbol segments")
        })
        .collect::<Vec<_>>();
    vec![
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            std::iter::once(
                ProjectSymbolSegment::try_new("character")
                    .expect("character namespace is a valid project symbol segment"),
            )
            .chain(compact_segments.iter().cloned()),
        )
        .expect("qualified character path has a valid implicit root"),
        ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact_segments)
            .expect("compact character path has a valid implicit root"),
    ]
}

pub(crate) fn one_character_facts(
    root: &Arc<SourceDocument>,
    world: ProjectSymbolWorldId,
    manifest: &CharacterManifest,
) -> ProjectRegistrationFacts {
    one_character_facts_with_documents(root, vec![Arc::clone(root)], world, manifest)
}

pub(crate) fn one_character_facts_with_documents(
    root: &Arc<SourceDocument>,
    mut documents: Vec<Arc<SourceDocument>>,
    world: ProjectSymbolWorldId,
    manifest: &CharacterManifest,
) -> ProjectRegistrationFacts {
    let (document, manifest) = backed_manifest(
        "arcweft-project://registration-tests/characters/akane.awchar.json",
        manifest,
    );
    let owner = manifest.manifest().character().clone();
    let declaration = declaration_span(&manifest);
    let binding_paths = character_binding_paths(&owner);
    let fact = external_fact(
        owner.as_str(),
        &binding_paths,
        RegisteredExternalOwner::Character(owner.clone()),
        declaration,
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![manifest])
        .expect("catalog");
    documents.push(document);
    ProjectRegistrationFacts::try_new(world, documents, vec![fact], vec![catalog])
        .expect("registration facts")
}

pub(crate) fn register(
    project: &HirProject,
    facts: &ProjectRegistrationFacts,
    base: TypeCheckEnv,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredSemanticWorld, crate::registration::CharacterRegistrationReport> {
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        project,
        facts,
        previous,
    ))
}

pub(crate) struct CharacterProjectFixture {
    source: Arc<SourceDocument>,
    module: CanonicalModulePath,
    _project: HirProject,
    _facts: ProjectRegistrationFacts,
    world: RegisteredSemanticWorld,
}

impl CharacterProjectFixture {
    pub(crate) fn new(source: &str) -> Self {
        let (documents, project, world_id) = project_modules(
            "character-definition",
            &[("", source), ("cast", "pub use crate.akane\n")],
        );
        let document = Arc::clone(documents.first().expect("fixture root document"));
        let manifest = sample_manifest("layers/body.png");
        let character = manifest.character().clone();
        let facts = one_character_facts_with_documents(&document, documents, world_id, &manifest);
        let base = TypeCheckEnv::standard()
            .with_function_signature(
                "accept_owner",
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "value",
                        TypeKind::entity_ref(EntityKind::Character),
                    )],
                ),
            )
            .with_function_signature(
                "accept_look",
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "value",
                        TypeKind::character_look(character.clone()),
                    )],
                ),
            )
            .with_function_signature(
                "accept_part",
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "value",
                        TypeKind::character_part(character.clone()),
                    )],
                ),
            )
            .with_function_signature(
                "accept_variant",
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "value",
                        TypeKind::character_variant(
                            character,
                            CharacterPartId::try_new("body").expect("body part"),
                        ),
                    )],
                ),
            )
            .with_function_signature(
                "accept_any",
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "value",
                        TypeKind::Named("Variant".to_owned()),
                    )],
                ),
            );
        let world = register(&project, &facts, base, None).expect("fixture world registers");
        Self {
            source: document,
            module: CanonicalModulePath::crate_root(),
            _project: project,
            _facts: facts,
            world,
        }
    }

    pub(crate) fn source(&self) -> &Arc<SourceDocument> {
        &self.source
    }

    pub(crate) const fn world(&self) -> &RegisteredSemanticWorld {
        &self.world
    }

    #[allow(
        clippy::result_large_err,
        reason = "the fixture exercises the production inventory error without changing its owned identity payload"
    )]
    pub(crate) fn collect(
        &self,
        budget: &mut CharacterDefinitionRequestBudget,
    ) -> Result<CharacterReferenceInventory, CharacterReferenceInventoryError> {
        self.collect_with_world(&self.world, budget)
    }

    #[allow(
        clippy::result_large_err,
        reason = "the fixture exercises the production inventory error without changing its owned identity payload"
    )]
    pub(crate) fn collect_with_world(
        &self,
        world: &RegisteredSemanticWorld,
        budget: &mut CharacterDefinitionRequestBudget,
    ) -> Result<CharacterReferenceInventory, CharacterReferenceInventoryError> {
        let parsed = parse_source(self.source.text());
        let hir = lower_document_to_hir(&self.source, parsed.typed_tree())
            .expect("fixture source lowers to document-bound HIR");
        let project = HirProject::new(
            PACKAGE,
            [
                HirProjectModule::try_new(self.module.clone(), self.source.identity().clone(), hir)
                    .expect("fixture module keeps canonical provenance"),
            ],
        )
        .expect("fixture request project");
        let linked = project.linked_module();
        let report = analyze_registered_project_types(&linked, world);
        collect_character_references(
            world,
            CharacterReferenceInput::new(
                &self.source,
                &self.module,
                parsed.typed_tree(),
                &report,
                parsed.errors(),
                None,
            ),
            budget,
        )
    }
}
