use std::sync::{Arc, atomic::AtomicBool};

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
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableCandidateId, CallableDiagnosticCode,
        CallableEffectSchema, CallableFamily, CallableGroupIndex, CallableGroupKind,
        CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameter,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallablePath, CallableSignatureSchema,
        CallableValidator, EnvironmentCallableKind, EnvironmentCallableOwner,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
        PRODUCTION_CALLABLE_LIMITS, PresentationCallableId, ProjectCallablePath,
        SemanticSignatureHelp, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    env::{
        EnumVariantPayload, FunctionParam, FunctionSignature, TypeCheckEnv,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
            AcceptedNominalRecord, AcceptedNominalSemantics,
        },
    },
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, EnvironmentCallableLookupInput,
        EnvironmentCallablePublicationMetadataInput, EnvironmentCallablePublicationRecordInput,
        EnvironmentCallableSignatureInput, EnvironmentManifestDigest,
        EnvironmentParameterGroupInput, EnvironmentParameterInput,
        EnvironmentParameterMetadataInput, EnvironmentParameterTypeInput,
        EnvironmentPublicationItemId, EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode,
        ExternalRegistrationFact, ProjectRegistrationFacts, RegisteredExternalOwner,
        RegisteredSemanticWorld, SourceBackedEnvironmentRegistrationInput,
    },
    signature::{
        SignatureNotApplicable, SignatureQuery, SignatureQueryControl, SignatureQueryOutcome,
        query_signature,
    },
    types::{CharacterNominalType, EntityKind, TypeKind},
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
    types::{TypePath, TypeRef, parse_type_ref},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSpan};

struct SignatureFixture {
    document: Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
}

struct TestPublication {
    owner: EnvironmentCallableOwner,
    records: Vec<EnvironmentCallablePublicationRecord>,
}

fn type_path(source: &str) -> TypePath {
    let authored = parse_type_ref(source).expect("fixture type path parses");
    let TypeRef::Path(path) = authored.value() else {
        panic!("fixture type path is direct")
    };
    path.clone()
}

impl SignatureFixture {
    fn new(
        name: &str,
        source: &str,
        environment: TypeCheckEnv,
        publications: Vec<TestPublication>,
    ) -> Self {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("memory:///{name}.arcw")).expect("source ID"),
                SourceName::path(format!("memory:///{name}.arcw")),
                source,
            )
            .expect("source document"),
        );
        let parsed = parse_source(source);
        assert!(
            parsed.errors().is_empty(),
            "signature matrix fixture must parse: {:?}",
            parsed.errors()
        );
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers");
        let package = CallablePackageId::try_new(name).expect("package");
        let project = HirProject::new(
            package.as_str(),
            [HirProjectModule::try_new(
                CanonicalModulePath::crate_root(),
                document.identity().clone(),
                hir,
            )
            .expect("root module")],
        )
        .expect("HIR project");
        let world_id = ProjectSymbolWorldId::try_new(
            package,
            document.identity().id().clone(),
            "call-surface-signature-matrix",
        )
        .expect("symbol world");
        let manifest = character_manifest();
        let mut environment_documents = Vec::new();
        let mut environment_inputs = Vec::new();
        for (index, publication) in publications.into_iter().enumerate() {
            let environment_document = Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new(format!("memory:///{name}-environment-{index}"))
                        .expect("environment source ID"),
                    SourceName::Generated,
                    "environment publication",
                )
                .expect("environment source"),
            );
            environment_inputs.push(environment_input(publication, &environment_document));
            environment_documents.push(environment_document);
        }
        let registration = registration_facts(
            &document,
            world_id,
            &manifest,
            environment_documents,
            environment_inputs,
        );
        let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(environment),
            &project,
            &registration,
            None,
        ))
        .expect("registered semantic world");
        Self {
            document,
            project,
            world,
        }
    }

    fn outcome(&self, call: &str, cursor: &str) -> SignatureQueryOutcome {
        let call_start = unique_offset(self.document.text(), call);
        let cursor = call_start
            + call
                .find(cursor)
                .unwrap_or_else(|| panic!("cursor marker {cursor:?} belongs to {call:?}"));
        let cancelled = AtomicBool::new(false);
        query_signature(
            SignatureQuery::production(
                &self.world,
                &self.document,
                &self.project.linked_module(),
                cursor,
                SignatureQueryControl::new(&cancelled, None),
            )
            .expect("fixture retains one accepted document/HIR/world tuple"),
        )
        .expect("signature query succeeds")
    }

    fn help(&self, call: &str, cursor: &str) -> SemanticSignatureHelp {
        let SignatureQueryOutcome::Help(help) = self.outcome(call, cursor) else {
            panic!("{call:?} must produce semantic signature help")
        };
        let expected = SourceRange::new(
            unique_offset(self.document.text(), call),
            unique_offset(self.document.text(), call) + call.len(),
        );
        assert_eq!(help.document(), self.document.identity());
        assert_eq!(help.call_span().range(), expected);
        help
    }
}

fn character_manifest() -> CharacterManifest {
    let character = CharacterId::try_new("character.alice").expect("character ID");
    let part = CharacterPartId::try_new("face").expect("part ID");
    let variant = CharacterVariantId::try_new("happy").expect("variant ID");
    let look = CharacterLookId::try_new("happy").expect("look ID");
    CharacterManifest::new(
        character,
        CharacterCanvas::new(32, 64),
        CharacterPoint::new(16, 64),
        look.clone(),
        vec![CharacterPart::new(
            part.clone(),
            0,
            vec![CharacterVariant::new(
                variant.clone(),
                CharacterAssetPath::try_new("layers/alice-happy.png").expect("asset path"),
                CharacterRect::new(0, 0, 32, 64),
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
    .expect("character manifest")
}

fn registration_facts(
    source: &Arc<SourceDocument>,
    world: ProjectSymbolWorldId,
    manifest: &CharacterManifest,
    environment_documents: Vec<Arc<SourceDocument>>,
    environment_inputs: Vec<SourceBackedEnvironmentRegistrationInput>,
) -> ProjectRegistrationFacts {
    let manifest_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///character-alice.json").expect("manifest ID"),
            SourceName::path("memory:///character-alice.json"),
            manifest.to_json_pretty().expect("manifest JSON"),
        )
        .expect("manifest document"),
    );
    let source_backed = SourceBackedCharacterManifest::decode_registration_json(&manifest_document)
        .expect("source-backed manifest");
    let owner = source_backed.manifest().character().clone();
    let declaration = source_backed
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("character token")
        .value()
        .clone();
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
            .expect("character path"),
        Some(Visibility::Public),
        declaration.clone(),
        character_bindings(&declaration),
    )
    .expect("character declaration");
    let mut documents = vec![Arc::clone(source), Arc::clone(&manifest_document)];
    documents.extend(environment_documents);
    ProjectRegistrationFacts::try_new(
        world,
        documents,
        vec![ExternalRegistrationFact::new(
            seed,
            RegisteredExternalOwner::Character(owner),
            declaration,
        )],
        vec![
            SourceBackedCharacterCatalog::try_new(source.identity().clone(), vec![source_backed])
                .expect("character catalog"),
        ],
        environment_inputs,
    )
    .expect("registration facts")
}

fn character_bindings(declaration: &SourceSpan) -> Vec<ProjectDirectBinding> {
    [
        (
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [
                    ProjectSymbolSegment::try_new("character").expect("namespace"),
                    ProjectSymbolSegment::try_new("alice").expect("character segment"),
                ],
            )
            .expect("canonical character binding"),
            false,
        ),
        (
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new("alice").expect("compact character")],
            )
            .expect("compact character binding"),
            false,
        ),
        (
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new("hero").expect("authored alias")],
            )
            .expect("authored alias binding"),
            true,
        ),
    ]
    .into_iter()
    .map(|(path, authored_alias)| {
        ProjectDirectBinding::try_new(
            CanonicalModulePath::crate_root(),
            path,
            Some(Visibility::Public),
            declaration.clone(),
            authored_alias,
        )
        .expect("character direct binding")
    })
    .collect()
}

fn unique_offset(source: &str, needle: &str) -> usize {
    let mut matches = source.match_indices(needle);
    let (offset, _) = matches
        .next()
        .unwrap_or_else(|| panic!("missing source text {needle:?}"));
    assert!(
        matches.next().is_none(),
        "source text must be unique: {needle:?}"
    );
    offset
}

fn active_parameter(help: &SemanticSignatureHelp) -> (&str, &CallableParameterType) {
    let active_signature = &help.signatures()[help.active_signature().get()];
    let coordinate = help.active_parameter().expect("active parameter");
    let parameter = &active_signature.groups()[coordinate.group().get()].parameters()
        [coordinate.parameter().get()];
    (
        parameter
            .name()
            .expect("matrix parameters are named")
            .as_str(),
        parameter.ty(),
    )
}

fn active_parameter_type(help: &SemanticSignatureHelp) -> &CallableParameterType {
    let active_signature = &help.signatures()[help.active_signature().get()];
    let coordinate = help.active_parameter().expect("active parameter");
    active_signature.groups()[coordinate.group().get()].parameters()[coordinate.parameter().get()]
        .ty()
}

fn parameter<'a>(help: &'a SemanticSignatureHelp, name: &str) -> &'a CallableParameterType {
    help.signatures()[help.active_signature().get()]
        .groups()
        .iter()
        .flat_map(arcweft_lang_sema::callable::SemanticParameterGroup::parameters)
        .find(|parameter| {
            parameter
                .name()
                .is_some_and(|candidate| candidate.as_str() == name)
        })
        .unwrap_or_else(|| panic!("missing semantic parameter {name:?}"))
        .ty()
}

fn presentation_candidate(help: &SemanticSignatureHelp) -> PresentationCallableId {
    let candidate = help.signatures()[help.active_signature().get()].candidate();
    let CallableCandidateId::Presentation(candidate) = candidate else {
        panic!("expected presentation candidate, got {candidate:?}")
    };
    candidate.to_owned()
}

#[test]
fn s01_s13_presentation_surfaces_retain_parser_spans_and_typed_active_parameters() {
    const SOURCE: &str = r"
fn main() -> Unit {
    let s01 = show(@character.alice, )
    let s02 = show( @character.alice)
    let s03 = show(@character.alice, look = .happy)
    let s04 = show(@character.alice, target = @target.stage)
    let s05 = hide(@character.alice)
    let s06 = ref.show(@character.alice)
    let s07 = view(@view.dialogue)
    let s08 = menu(@view.menu, depth = 0i32)
    let s09 = overlay(@view.overlay, visible = true)
    let s10 = bg(@asset.room)
    let s11 = image(@asset.hero, opacity = 1.0)
    let s12 = player_viewport(width = 1280i32)
    let s13 = clear.bg(target = @target.main)
    let alias = show(hero, look = .happy)
    ()
}
";
    let fixture = SignatureFixture::new(
        "surface-presentation",
        SOURCE,
        TypeCheckEnv::standard(),
        Vec::new(),
    );
    let alice = TypeKind::character_look(
        CharacterId::try_new("character.alice").expect("Alice character ID"),
    );

    assert_typed_show_surfaces(&fixture, &alice);
    assert_other_typed_presentation_surfaces(&fixture);
}

fn assert_typed_show_surfaces(fixture: &SignatureFixture, alice: &TypeKind) {
    let s01 = fixture.help("show(@character.alice, )", ")");
    assert_eq!(presentation_candidate(&s01), PresentationCallableId::Show);
    assert_eq!(
        active_parameter(&s01),
        ("look", &CallableParameterType::Exact(alice.clone()))
    );

    let s02 = fixture.help("show( @character.alice)", "@character.alice");
    assert_eq!(active_parameter(&s02).0, "character");
    assert_eq!(
        parameter(&s02, "look"),
        &CallableParameterType::Exact(alice.clone())
    );

    for (call, cursor, active) in [
        ("show(@character.alice, look = .happy)", ".happy", "look"),
        (
            "show(@character.alice, target = @target.stage)",
            "@target.stage",
            "target",
        ),
    ] {
        let help = fixture.help(call, cursor);
        assert_eq!(presentation_candidate(&help), PresentationCallableId::Show);
        assert_eq!(active_parameter(&help).0, active);
        assert_eq!(
            parameter(&help, "look"),
            &CallableParameterType::Exact(alice.clone())
        );
    }

    let alias = fixture.help("show(hero, look = .happy)", ".happy");
    assert_eq!(presentation_candidate(&alias), PresentationCallableId::Show);
    assert_eq!(
        active_parameter(&alias),
        ("look", &CallableParameterType::Exact(alice.clone()))
    );

    for (call, cursor, candidate) in [
        (
            "hide(@character.alice)",
            "@character.alice",
            PresentationCallableId::Hide,
        ),
        (
            "ref.show(@character.alice)",
            "@character.alice",
            PresentationCallableId::RefShow,
        ),
    ] {
        let help = fixture.help(call, cursor);
        assert_eq!(presentation_candidate(&help), candidate);
        assert_eq!(active_parameter(&help).0, "character");
        assert!(
            help.signatures()[help.active_signature().get()]
                .groups()
                .iter()
                .flat_map(arcweft_lang_sema::callable::SemanticParameterGroup::parameters)
                .all(|parameter| parameter.name().is_none_or(|name| name.as_str() != "look"))
        );
    }
}

fn assert_other_typed_presentation_surfaces(fixture: &SignatureFixture) {
    let typed_cases = [
        (
            "view(@view.dialogue)",
            "@view.dialogue",
            PresentationCallableId::View,
            "view",
            CallableParameterType::Exact(TypeKind::entity_ref(EntityKind::View)),
        ),
        (
            "menu(@view.menu, depth = 0i32)",
            "0i32",
            PresentationCallableId::Menu,
            "depth",
            CallableParameterType::Exact(TypeKind::I32),
        ),
        (
            "overlay(@view.overlay, visible = true)",
            "true",
            PresentationCallableId::Overlay,
            "visible",
            CallableParameterType::Exact(TypeKind::Bool),
        ),
        (
            "bg(@asset.room)",
            "@asset.room",
            PresentationCallableId::Background,
            "asset",
            CallableParameterType::Exact(TypeKind::entity_ref(EntityKind::Asset)),
        ),
        (
            "clear.bg(target = @target.main)",
            "@target.main",
            PresentationCallableId::ClearBackground,
            "target",
            CallableParameterType::Exact(TypeKind::entity_ref(EntityKind::Target)),
        ),
    ];
    for (call, cursor, candidate, name, ty) in typed_cases {
        let help = fixture.help(call, cursor);
        assert_eq!(presentation_candidate(&help), candidate);
        assert_eq!(active_parameter(&help), (name, &ty));
    }

    let image = fixture.help("image(@asset.hero, opacity = 1.0)", "1.0");
    assert_eq!(
        presentation_candidate(&image),
        PresentationCallableId::Image
    );
    assert_eq!(active_parameter(&image).0, "opacity");
    assert_eq!(
        active_parameter(&image).1,
        &CallableParameterType::Exact(TypeKind::Choice(vec![
            TypeKind::I32,
            TypeKind::F64,
            TypeKind::String,
        ]))
    );

    let viewport = fixture.help("player_viewport(width = 1280i32)", "1280i32");
    assert_eq!(
        presentation_candidate(&viewport),
        PresentationCallableId::PlayerViewport
    );
    assert_eq!(active_parameter(&viewport).0, "width");
    assert_eq!(
        active_parameter(&viewport).1,
        &CallableParameterType::Exact(TypeKind::Choice(vec![
            TypeKind::Named("Length".to_owned()),
            TypeKind::I32,
            TypeKind::F64,
            TypeKind::String,
        ]))
    );
}

#[test]
fn c10_c11_unavailable_show_owners_remain_unchecked_without_nominal_fabrication() {
    const SOURCE: &str = r"
fn main() -> Unit {
    let missing = show(look = .happy)
    let non_character = show(@view.dialogue, look = .happy)
    ()
}
";
    let fixture = SignatureFixture::new(
        "surface-unavailable-presentation-owner",
        SOURCE,
        TypeCheckEnv::standard(),
        Vec::new(),
    );

    let missing = fixture.help("show(look = .happy)", ".happy");
    assert_eq!(
        presentation_candidate(&missing),
        PresentationCallableId::Show
    );
    assert_eq!(
        parameter(&missing, "look"),
        &CallableParameterType::Unchecked
    );
    assert!(
        missing
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == CallableDiagnosticCode::MissingArgument)
    );

    let non_character = fixture.help("show(@view.dialogue, look = .happy)", ".happy");
    assert_eq!(
        presentation_candidate(&non_character),
        PresentationCallableId::Show
    );
    assert_eq!(
        parameter(&non_character, "look"),
        &CallableParameterType::Unchecked
    );
}

#[test]
fn s17_s18_character_dialogue_non_argument_positions_do_not_produce_help() {
    const SOURCE: &str = r"
flow @flow.main main {
    alice: Dialogue content.
    alice[Bracket content.]
}
";
    let fixture = SignatureFixture::new(
        "surface-dialogue-boundaries",
        SOURCE,
        TypeCheckEnv::standard(),
        Vec::new(),
    );
    assert_eq!(
        fixture.outcome("alice: Dialogue content.", ":"),
        SignatureQueryOutcome::NotApplicable(SignatureNotApplicable::CursorOutsideArgumentList)
    );
    assert!(matches!(
        fixture.outcome("alice: Dialogue content.", "Dialogue"),
        SignatureQueryOutcome::NotApplicable(
            SignatureNotApplicable::CursorOutsideArgumentList
                | SignatureNotApplicable::UnsupportedSurface
        )
    ));
    assert!(matches!(
        fixture.outcome("alice[Bracket content.]", "Bracket"),
        SignatureQueryOutcome::NotApplicable(
            SignatureNotApplicable::CursorOutsideArgumentList
                | SignatureNotApplicable::UnsupportedSurface
        )
    ));
}

#[test]
fn s20_s21_selected_environment_method_is_typed_and_unknown_method_is_not_synthesized() {
    const SOURCE: &str = r"
struct Actor { stage: i32 }

fn main(actor: Actor) -> Unit {
    let shown: String = actor.stage.show(1i32)
    let missing = actor.stage.move(2i32)
    ()
}
";
    let environment = TypeCheckEnv::standard().with_method_signature(
        TypeKind::I32,
        "show",
        FunctionSignature::new(
            TypeKind::String,
            [FunctionParam::required("look", TypeKind::I32)],
        ),
    );
    let fixture = SignatureFixture::new("surface-methods", SOURCE, environment, Vec::new());
    let show = fixture.help("actor.stage.show(1i32)", "1i32");
    assert!(matches!(
        show.signatures()[show.active_signature().get()].candidate(),
        CallableCandidateId::Environment(_)
    ));
    assert_eq!(
        active_parameter(&show),
        ("look", &CallableParameterType::Exact(TypeKind::I32))
    );
    assert_eq!(
        fixture.outcome("actor.stage.move(2i32)", "2i32"),
        SignatureQueryOutcome::NotApplicable(SignatureNotApplicable::UnknownCallee)
    );
}

#[test]
fn s22_s23_source_owned_callable_families_require_accepted_nominal_evidence() {
    const SOURCE: &str = r"
extern capability character_host {
    fn accept_look(look: AliceLook) -> Unit
}

fn project_look(look: AliceLook) -> Unit {
    ()
}

fn main() -> Unit {
    project_look(alice_look_value())
    character_host.accept_look(alice_look_value())
    ()
}
";
    let alice_id = CharacterId::try_new("character.alice").expect("Alice character ID");
    let alice = TypeKind::character_look(alice_id.clone());
    let environment = TypeCheckEnv::standard()
        .try_with_nominal_record(
            AcceptedNominalRecord::try_new(
                AcceptedNominalId::new(
                    AcceptedNominalOwnerId::Character(alice_id.clone()),
                    type_path("AliceLook"),
                ),
                0,
                AcceptedNominalSemantics::Character(CharacterNominalType::Look {
                    character: alice_id,
                }),
                AcceptedNominalOrigin::Character,
                None,
            )
            .expect("Alice look nominal record"),
        )
        .expect("Alice look nominal registers")
        .with_function("alice_look_value", alice.clone());
    let fixture =
        SignatureFixture::new("surface-source-callables", SOURCE, environment, Vec::new());
    for (call, cursor) in [
        ("project_look(alice_look_value())", "alice_look_value"),
        (
            "character_host.accept_look(alice_look_value())",
            "alice_look_value",
        ),
    ] {
        let help = fixture.help(call, cursor);
        assert!(matches!(
            help.signatures()[help.active_signature().get()].candidate(),
            CallableCandidateId::Project(_)
        ));
        assert_eq!(
            active_parameter(&help),
            ("look", &CallableParameterType::Exact(alice.clone()))
        );
    }
}

#[test]
fn s24_s27_registered_nominal_surfaces_share_structural_character_identity() {
    const SOURCE: &str = r"
fn main() -> Unit {
    adapter_nominal(alice_look_value())
    consume_choice(CharacterChoice.Look(alice_look_value()))
    consume_result(Ok(alice_look_value()))
    let callback = nominal_callback()
    callback(alice_look_value())
    ()
}
";
    let alice = TypeKind::character_look(
        CharacterId::try_new("character.alice").expect("Alice character ID"),
    );
    let choice = TypeKind::Named("CharacterChoice".to_owned());
    let result = TypeKind::Result {
        ok: Box::new(alice.clone()),
        error: Box::new(TypeKind::String),
    };
    let environment = TypeCheckEnv::standard()
        .with_function("alice_look_value", alice.clone())
        .with_function_signature(
            "consume_choice",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("choice", choice.clone())],
            ),
        )
        .with_function_signature(
            "consume_result",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("result", result.clone())],
            ),
        )
        .with_function(
            "nominal_callback",
            TypeKind::function([alice.clone()], TypeKind::Unit),
        )
        .try_with_enum_variant_payload(choice, "Look", EnumVariantPayload::tuple([alice.clone()]))
        .expect("nominal enum payload");
    let fixture = SignatureFixture::new(
        "surface-nominal-families",
        SOURCE,
        environment,
        vec![adapter_nominal_publication(alice.clone())],
    );

    let adapter = fixture.help("adapter_nominal(alice_look_value())", "alice_look_value");
    assert!(matches!(
        adapter.signatures()[adapter.active_signature().get()].candidate(),
        CallableCandidateId::Environment(id)
            if matches!(id.owner(), EnvironmentCallableOwner::Adapter(_))
    ));
    assert_eq!(
        active_parameter(&adapter),
        ("look", &CallableParameterType::Exact(alice.clone()))
    );

    let variant = fixture.help(
        "CharacterChoice.Look(alice_look_value())",
        "alice_look_value",
    );
    assert!(matches!(
        variant.signatures()[variant.active_signature().get()].candidate(),
        CallableCandidateId::EnumVariant(_)
    ));
    assert_eq!(
        active_parameter_type(&variant),
        &CallableParameterType::Exact(alice.clone())
    );

    let ok = fixture.help("Ok(alice_look_value())", "alice_look_value");
    assert!(matches!(
        ok.signatures()[ok.active_signature().get()].candidate(),
        CallableCandidateId::Result(_)
    ));
    assert_eq!(
        active_parameter(&ok),
        ("payload", &CallableParameterType::Exact(alice.clone()))
    );
    assert_eq!(
        ok.signatures()[ok.active_signature().get()].result(),
        &result
    );

    let function_value = fixture.help("callback(alice_look_value())", "alice_look_value");
    assert!(matches!(
        function_value.signatures()[function_value.active_signature().get()].candidate(),
        CallableCandidateId::FunctionValue(_)
    ));
    assert_eq!(
        active_parameter(&function_value),
        ("arg1", &CallableParameterType::Exact(alice))
    );
}

#[test]
fn s30_superseded_project_method_premise_uses_the_accepted_trait_catalog() {
    const SOURCE: &str = r#"
struct ActorStage {}
struct Actor { stage: ActorStage }

trait StageMotion {
    fn move(self, distance: i32) -> String
}

impl StageMotion for ActorStage {
    fn move(self, distance: i32) -> String {
        "moved"
    }
}

fn main(actor: Actor) -> Unit {
    let moved: String = actor.stage.move(2i32)
    ()
}
"#;
    let fixture = SignatureFixture::new(
        "surface-trait-method",
        SOURCE,
        TypeCheckEnv::standard(),
        Vec::new(),
    );
    let help = fixture.help("actor.stage.move(2i32)", "2i32");
    assert_eq!(
        help.signatures()[help.active_signature().get()]
            .candidate()
            .family(),
        CallableFamily::TraitMethod
    );
    assert_eq!(
        active_parameter(&help),
        ("distance", &CallableParameterType::Exact(TypeKind::I32))
    );
}

fn adapter_nominal_publication(nominal: TypeKind) -> TestPublication {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter zero"),
        Some(CallableName::try_new("look").expect("parameter name")),
        CallableParameterType::Exact(nominal),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        None,
    )
    .expect("adapter parameter");
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::try_from_usize(0).expect("group zero"),
        CallableGroupKind::Initial,
        vec![parameter],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("adapter group");
    let schema = CallableSignatureSchema::try_new(
        vec![group],
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("adapter schema");
    let record = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(
            CallablePath::try_new([
                CallableName::try_new("adapter_nominal").expect("callable name")
            ])
            .expect("callable path"),
        ),
        CallableOverloadIndex::try_from_usize(0).expect("overload zero"),
        schema,
        arcweft_lang_sema::callable::CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(0).expect("ordinal zero"),
    )
    .expect("adapter record");
    TestPublication {
        owner: EnvironmentCallableOwner::Adapter(
            AdapterPackageId::try_new("adapter.nominal-surface").expect("adapter ID"),
        ),
        records: vec![record],
    }
}

fn environment_input(
    publication: TestPublication,
    source: &SourceDocument,
) -> SourceBackedEnvironmentRegistrationInput {
    let span = source
        .span(SourceRange::new(0, source.text().len()))
        .expect("environment source span");
    let package = CallablePackageId::try_new(match &publication.owner {
        EnvironmentCallableOwner::Adapter(owner) => owner.as_str(),
        EnvironmentCallableOwner::Standard(_) => "standard-signature-matrix",
    })
    .expect("environment package");
    let records = publication
        .records
        .into_iter()
        .map(|record| {
            let CallableLookupKey::Free(path) = record.key() else {
                panic!("signature matrix publishes only free callables")
            };
            let path = ProjectCallablePath::new(
                package.clone(),
                CanonicalModulePath::crate_root(),
                path.clone(),
            );
            let item = EnvironmentPublicationItemId::AdapterFunction {
                owner: publication.owner.clone(),
                path: path.clone(),
                overload: record.overload(),
            };
            let groups = record
                .schema()
                .groups()
                .iter()
                .map(|group| {
                    EnvironmentParameterGroupInput::new(
                        group.index(),
                        group.kind(),
                        group
                            .parameters()
                            .iter()
                            .map(|parameter| {
                                EnvironmentParameterInput::new(
                                    parameter.index(),
                                    parameter.name().cloned(),
                                    match parameter.ty() {
                                        CallableParameterType::Exact(ty) => {
                                            EnvironmentParameterTypeInput::Exact(neutral_type(
                                                ty, &span,
                                            ))
                                        }
                                        CallableParameterType::Unchecked => {
                                            EnvironmentParameterTypeInput::Unchecked {
                                                source: span.clone(),
                                            }
                                        }
                                    },
                                    parameter.passing(),
                                    parameter.presence(),
                                    EnvironmentParameterMetadataInput::new(
                                        parameter.documentation().map(Into::into),
                                        None,
                                    ),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            EnvironmentCallablePublicationRecordInput::new(
                item,
                record.kind(),
                EnvironmentCallableLookupInput::Free(path),
                record.overload(),
                EnvironmentCallableSignatureInput::new(
                    groups,
                    neutral_type(record.schema().result(), &span),
                    record.schema().effects().declared().clone(),
                    record.schema().argument_policy(),
                    record.schema().validator().clone(),
                ),
                record.declaration_order(),
                EnvironmentCallablePublicationMetadataInput::new(
                    record.documentation().clone(),
                    record.source().cloned(),
                    record.rust().cloned(),
                ),
            )
        })
        .collect::<Vec<_>>();
    SourceBackedEnvironmentRegistrationInput::new(
        publication.owner,
        source.identity().clone(),
        EnvironmentManifestDigest::from_bytes(*blake3::hash(source.text().as_bytes()).as_bytes()),
        [],
        [],
        [],
        records,
    )
}

fn neutral_type(ty: &TypeKind, source: &SourceSpan) -> EnvironmentTypeProjectionNode {
    let kind = match ty {
        TypeKind::Unit => EnvironmentTypeProjectionKind::Unit,
        TypeKind::Bool => EnvironmentTypeProjectionKind::Bool,
        TypeKind::I32 => EnvironmentTypeProjectionKind::I32,
        TypeKind::String => EnvironmentTypeProjectionKind::String,
        TypeKind::CharacterNominal(nominal) => {
            EnvironmentTypeProjectionKind::CharacterNominal(nominal.clone())
        }
        TypeKind::AcceptedNominal(nominal) => EnvironmentTypeProjectionKind::AcceptedNominal {
            id: nominal.declaration().clone(),
            arguments: nominal
                .arguments()
                .iter()
                .map(|argument| neutral_type(argument, source))
                .collect(),
        },
        other => panic!("unsupported signature-matrix environment type: {other:?}"),
    };
    EnvironmentTypeProjectionNode::new(source.clone(), kind)
}
