use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_adapter_context::manifest::{
    AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
    AdapterCallableParameterIndex, AdapterCallablePath, AdapterEnvironmentOwnerId,
    AdapterFunctionParam, AdapterFunctionSignature, AdapterManifest, AdapterNominalDeclaration,
    AdapterNominalOwner, AdapterNominalPath, AdapterNominalPathSegment, AdapterNominalTypeRef,
    AdapterNominalVisibility, AdapterOpaqueTypeProducerId, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterTypeKind,
};
use arcweft_adapter_sema::registration::AdapterSemanticRegistration;
use arcweft_compiler::incremental::{BuildSnapshotRequest, snapshot_compiled_project};
use arcweft_compiler::lower::{
    RuntimeEmissionMode, project_runtime_reachability, project_runtime_semantic_facts,
};
use arcweft_compiler::project::{
    CompiledProject, CompiledProjectModule, ProjectCompilationContext, ProjectCompilationSession,
    ProjectCompileCache, ProjectCompileCacheStatus, ProjectCompileError,
    ProjectCompileUnitFingerprint, compile_project_with_cache,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_hir::{item::HirItemKind, stmt::HirStmtKind};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    final_analysis::CheckedStatementPayload,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
    types::TypeKind,
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    incremental::{ParsedSource, SyntaxDatabase},
    parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::fingerprint::BuildDigest;
use arcweft_project::graph::ModuleDependency;
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_runtime_plan::{
    flow::{RuntimeEntryLoweringInput, lower_runtime_plan_with_stats},
    semantic_facts::{RuntimeSemanticTypeId, RuntimeTypeShape, RuntimeVariantOwner},
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange,
    identity::SourceSnapshotId,
};

#[derive(Default)]
struct RecordingCache {
    units: BTreeMap<ProjectCompileUnitFingerprint, Vec<CompiledProjectModule>>,
    loads: usize,
    stores: usize,
}

impl RecordingCache {
    fn reset_activity(&mut self) {
        self.loads = 0;
        self.stores = 0;
    }
}

impl ProjectCompileCache for RecordingCache {
    fn load(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
    ) -> Option<Vec<CompiledProjectModule>> {
        self.loads += 1;
        self.units.get(&fingerprint).cloned()
    }

    fn store(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
        modules: &[CompiledProjectModule],
    ) {
        self.stores += 1;
        self.units.insert(fingerprint, modules.to_vec());
    }
}

struct AttachedCompiler {
    session: ProjectCompilationSession,
    syntax: SyntaxDatabase,
    parsed_sources: BTreeMap<CanonicalModulePath, ParsedSource>,
}

impl AttachedCompiler {
    fn new(project: &ProjectSources) -> Self {
        let mut syntax = SyntaxDatabase::try_new().expect("cache test syntax database");
        let parsed_sources = parse_project_sources(&mut syntax, project);
        Self {
            session: ProjectCompilationSession::try_new().expect("cache test HIR database"),
            syntax,
            parsed_sources,
        }
    }

    fn replace_sources(&mut self, project: &ProjectSources) {
        let same_lineages = self.parsed_sources.len() == project.modules().len()
            && project.modules().all(|source| {
                self.parsed_sources
                    .get(source.module())
                    .is_some_and(|parsed| {
                        parsed.document().identity().id() == source.document().identity().id()
                    })
            });
        if !same_lineages {
            let mut syntax = SyntaxDatabase::try_new().expect("replacement syntax database");
            self.parsed_sources = parse_project_sources(&mut syntax, project);
            self.syntax = syntax;
            return;
        }

        let mut next = BTreeMap::new();
        for source in project.modules() {
            let parsed = match self.parsed_sources.get(source.module()) {
                Some(previous)
                    if previous.document().identity() == source.document().identity() =>
                {
                    previous.clone()
                }
                Some(previous) => {
                    let whole = previous
                        .document()
                        .span(SourceRange::new(0, previous.source().len()))
                        .expect("whole previous document span");
                    self.syntax
                        .reparse(
                            previous,
                            &[SourceEdit::new(whole, source.document().text())],
                            ParseOptions::default(),
                        )
                        .expect("incremental cache-test reparse")
                }
                None => self
                    .syntax
                    .parse_initial(
                        SourceSnapshotId::initial(source.document().display_name().clone()),
                        Arc::clone(source.document()),
                        ParseOptions::default(),
                    )
                    .expect("new cache-test attached source"),
            };
            next.insert(source.module().clone(), parsed);
        }
        self.parsed_sources = next;
    }

    fn compile<C: ProjectCompileCache>(
        &mut self,
        project: &ProjectSources,
        context: &ProjectCompilationContext,
        cache: &mut C,
    ) -> Result<CompiledProject, ProjectCompileError> {
        compile_project_with_cache(
            &mut self.session,
            project,
            &self.parsed_sources,
            context,
            cache,
        )
    }
}

fn parse_project_sources(
    syntax: &mut SyntaxDatabase,
    project: &ProjectSources,
) -> BTreeMap<CanonicalModulePath, ParsedSource> {
    project
        .modules()
        .map(|source| {
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(source.document().display_name().clone()),
                    Arc::clone(source.document()),
                    ParseOptions::default(),
                )
                .expect("cache test attached source");
            (source.module().clone(), parsed)
        })
        .collect()
}

fn fixture(source: &str, profile: &str) -> (ProjectSources, Arc<ProjectRegistrationFacts>) {
    let (project, document, world) = project_fixture(source, profile);
    (project, registration_facts(document, world))
}

fn registration_facts(
    document: Arc<SourceDocument>,
    world: ProjectSymbolWorldId,
) -> Arc<ProjectRegistrationFacts> {
    Arc::new(
        ProjectRegistrationFacts::try_new(
            world,
            vec![document],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("registration facts"),
    )
}

fn project_fixture(
    source: &str,
    profile: &str,
) -> (ProjectSources, Arc<SourceDocument>, ProjectSymbolWorldId) {
    project_fixture_with_document_id(
        source,
        profile,
        &format!("arcweft-project://compiler-cache-{profile}/src/main.arcw"),
    )
}

fn project_fixture_with_document_id(
    source: &str,
    profile: &str,
    document_id: &str,
) -> (ProjectSources, Arc<SourceDocument>, ProjectSymbolWorldId) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(document_id).expect("document id"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("document"),
    );
    let package_id = format!("org.arcweft.compiler-cache-{profile}");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new(package_id.clone()).expect("package ID"),
            version: PackageVersion::new("0.1.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!(
                    "arcweft-project://compiler-cache-{profile}/arcw.toml"
                ))
                .expect("manifest document ID"),
                SourceName::path("arcw.toml"),
                format!("schema = 1\n[package]\nid = \"{package_id}\"\nversion = \"0.1.0\"\n"),
            )
            .expect("manifest document"),
        ),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(package_id).expect("package"),
        document.identity().id().clone(),
        profile,
    )
    .expect("world");
    (project, document, world)
}

fn child_module(name: &str) -> CanonicalModulePath {
    CanonicalModulePath::crate_root().join(ModuleSegment::new(name).expect("module segment"))
}

fn three_unit_document(profile: &str, file: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-project://compiler-cache-{profile}/src/{file}.arcw"
            ))
            .expect("three-unit document ID"),
            SourceName::path(format!("src/{file}.arcw")),
            source,
        )
        .expect("three-unit source document"),
    )
}

fn three_unit_project(
    profile: &str,
    root: Arc<SourceDocument>,
    dependency: Arc<SourceDocument>,
    unrelated: Arc<SourceDocument>,
) -> (ProjectSources, Arc<ProjectRegistrationFacts>) {
    let package_id = format!("org.arcweft.compiler-cache-{profile}");
    let package = PackageSpec {
        id: PackageId::new(package_id.clone()).expect("package ID"),
        version: PackageVersion::new("0.1.0").expect("package version"),
    };
    let dependency_path = child_module("dependency");
    let unrelated_path = child_module("unrelated");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package,
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!(
                    "arcweft-project://compiler-cache-{profile}/arcw.toml"
                ))
                .expect("manifest document ID"),
                SourceName::path("arcw.toml"),
                format!("schema = 1\n[package]\nid = \"{package_id}\"\nversion = \"0.1.0\"\n"),
            )
            .expect("manifest document"),
        ),
        [
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::clone(&root),
                [ModuleDependency::new(dependency_path.clone())],
            ),
            ProjectSourceFile::new(
                dependency_path,
                PathBuf::from("src/dependency.arcw"),
                Arc::clone(&dependency),
                [],
            ),
            ProjectSourceFile::new(
                unrelated_path,
                PathBuf::from("src/unrelated.arcw"),
                Arc::clone(&unrelated),
                [],
            ),
        ],
    )
    .expect("three-unit project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(package_id).expect("package"),
        root.identity().id().clone(),
        profile,
    )
    .expect("three-unit symbol world");
    let facts = Arc::new(
        ProjectRegistrationFacts::try_new(
            world,
            vec![root, dependency, unrelated],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("three-unit registration facts"),
    );
    (project, facts)
}

fn fixture_with_manifest(
    source: &str,
    profile: &str,
    manifest: &AdapterManifest,
) -> (ProjectSources, Arc<ProjectRegistrationFacts>, TypeCheckEnv) {
    let (project, document, world) = project_fixture(source, profile);
    let parts = AdapterSemanticRegistration::new(manifest)
        .source_backed_facts(0)
        .expect("adapter registration facts")
        .into_parts();
    let facts = Arc::new(
        ProjectRegistrationFacts::try_new(
            world,
            vec![document, parts.document],
            parts.externals.into_vec(),
            Vec::new(),
            vec![parts.environment],
        )
        .expect("registration facts"),
    );
    (
        project,
        facts,
        AdapterSemanticRegistration::new(manifest).declare_effects(TypeCheckEnv::standard()),
    )
}

fn nominal_manifest(adapter: &str, path: &str, nested_option: bool) -> AdapterManifest {
    let manifest = AdapterManifest::new(adapter, "Persistent nominal fixture");
    let nominal_path = AdapterNominalPath::try_new([
        AdapterNominalPathSegment::try_new(path).expect("nominal path segment")
    ])
    .expect("nominal path");
    let owner = AdapterEnvironmentOwnerId::for_adapter(manifest.id());
    let nominal = AdapterTypeKind::Nominal {
        nominal: AdapterNominalTypeRef::try_new(
            AdapterNominalOwner::Environment { owner },
            nominal_path.clone(),
            [],
        )
        .expect("nominal reference"),
    };
    let parameter_type = if nested_option {
        AdapterTypeKind::Option {
            item: Box::new(nominal),
        }
    } else {
        nominal
    };
    let parameter = AdapterFunctionParam::try_new(
        AdapterCallableParameterIndex::try_from_usize(0).expect("parameter index"),
        Some(AdapterCallableName::try_new("value").expect("parameter name")),
        parameter_type,
        AdapterParameterPassing::PositionalOrNamed,
        AdapterParameterPresence::Required,
    )
    .expect("function parameter");
    let signature = AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0).expect("group index"),
                vec![parameter],
            )
            .expect("parameter group"),
        ],
        AdapterTypeKind::Unit,
    )
    .expect("function signature");

    manifest
        .try_with_nominal_declaration(
            AdapterNominalDeclaration::try_new(
                nominal_path,
                0,
                AdapterOpaqueTypeProducerId::try_new("fixture.project.external-types")
                    .expect("fixture producer is valid"),
                AdapterNominalVisibility::Public,
                "persistent nominal",
            )
            .expect("nominal declaration"),
        )
        .expect("unique nominal declaration")
        .with_function_signature(
            AdapterCallablePath::single(
                AdapterCallableName::try_new("accept").expect("callable name"),
            ),
            AdapterCallableOverloadIndex::try_from_usize(0).expect("overload index"),
            signature,
            [],
        )
}

fn snapshot_with_manifest(
    profile: &str,
    manifest: &AdapterManifest,
) -> arcweft_project::incremental::BuildSnapshot {
    let (project, facts, base) =
        fixture_with_manifest("fn main() -> Unit { () }\n", profile, manifest);
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(base, facts),
            &mut RecordingCache::default(),
        )
        .expect("compiled project");
    snapshot_compiled_project(
        &project,
        &compiled,
        BuildSnapshotRequest {
            build_id: "persistent-environment-build".to_owned(),
            compiler_build_id: "compiler-test".to_owned(),
            target_triple: "test-target".to_owned(),
            target_features: Vec::new(),
            profile: "test".to_owned(),
            selected_entries: Vec::new(),
        },
    )
}

fn context(base: TypeCheckEnv, facts: Arc<ProjectRegistrationFacts>) -> ProjectCompilationContext {
    ProjectCompilationContext::new(
        Arc::new(base),
        facts,
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
    )
}

#[test]
fn compiled_project_contains_no_linked_hir() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "shared-hir");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let root = CanonicalModulePath::crate_root();
    let expected_parsed = session
        .parsed_sources
        .get(&root)
        .expect("root parsed source")
        .clone();
    let expected_hir_database = session.session.hir_database_id();
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("compiled project");

    let retained = Arc::clone(compiled.hir_project());
    assert!(Arc::ptr_eq(compiled.hir_project(), &retained));
    let compiled_module = &compiled.modules()[0];
    let accepted_module = compiled
        .hir_project()
        .module(compiled_module.module())
        .expect("compiled module remains present in the accepted HIR project")
        .module();
    assert!(
        Arc::ptr_eq(compiled_module.hir(), accepted_module),
        "compiled modules retain the exact accepted project HIR lease instead of a linked clone",
    );
    assert!(compiled_module.parsed().is_same_snapshot(&expected_parsed));
    assert!(Arc::ptr_eq(
        compiled_module.parsed().document_lease(),
        expected_parsed.document_lease()
    ));
    assert_eq!(
        compiled_module.hir().provenance().syntax_snapshot(),
        expected_parsed.snapshot_id()
    );
    assert_eq!(
        compiled_module.hir().snapshot_id().module().database(),
        expected_hir_database
    );
    assert_eq!(
        compiled_module.source(),
        project.root_module().document().identity()
    );
}

#[test]
fn runtime_plan_consumes_project_view_without_flattening() {
    let profile = "runtime-project-view";
    let root = three_unit_document(profile, "main", "flow root {\n}\n");
    let dependency = three_unit_document(profile, "dependency", "flow dependency {\n}\n");
    let unrelated = three_unit_document(profile, "unrelated", "flow unrelated {\n}\n");
    let expected_paths = [
        CanonicalModulePath::crate_root(),
        child_module("dependency"),
        child_module("unrelated"),
    ];
    let (project, facts) = three_unit_project(profile, root, dependency, unrelated);
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("three-module project compiles");

    let executable = compiled
        .hir_project()
        .executable_view()
        .expect("accepted project is executable");
    let runtime_owners = project_runtime_reachability(
        executable,
        compiled.project_symbols(),
        compiled.final_analysis(),
        compiled.checked_entries(),
        RuntimeEmissionMode::CheckAll,
    )
    .expect("runtime reachability projects from the accepted project view");
    let runtime_facts = project_runtime_semantic_facts(
        executable,
        compiled.project_symbols(),
        compiled.registered_world(),
        compiled.final_analysis(),
        &runtime_owners,
        None,
        None,
    )
    .expect("runtime facts project from the accepted project view");
    let entry_input = RuntimeEntryLoweringInput::empty(executable);
    let lowered = lower_runtime_plan_with_stats(executable, &runtime_facts, &entry_input)
        .expect("runtime plan lowers from the accepted project view");

    let projected = executable
        .items()
        .filter_map(|item| {
            if !matches!(item.item().kind(), HirItemKind::Flow(_)) {
                return None;
            }
            let accepted_module = executable
                .module(item.module_path())
                .expect("project item retains its canonical module");
            assert!(Arc::ptr_eq(item.module(), accepted_module));
            assert_eq!(item.id().module(), item.module().module_id());
            assert!(compiled.final_analysis().item(item.id()).is_some());
            let runtime = runtime_facts
                .flow(item.id())
                .cloned()
                .expect("module-qualified flow retains its runtime projection");
            Some((item.module_path().clone(), item.id(), runtime))
        })
        .collect::<Vec<_>>();

    assert_eq!(
        projected
            .iter()
            .map(|(path, _, _)| path.clone())
            .collect::<Vec<_>>(),
        expected_paths
    );
    assert!(projected.iter().enumerate().all(|(index, (_, item, _))| {
        projected[..index]
            .iter()
            .all(|(_, prior, _)| item.module() != prior.module())
    }));
    assert_eq!(
        lowered
            .plan
            .flows()
            .iter()
            .map(|flow| flow.id.clone())
            .collect::<Vec<_>>(),
        projected
            .iter()
            .map(|(_, _, runtime)| runtime.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one compiler-boundary fixture compares every normalized type owner family across the runtime and presentation domains"
)]
fn runtime_semantic_facts_retain_exact_runtime_domain_types_and_omit_presentation_owners() {
    let (project, facts) = fixture(
        concat!(
            "fn main(flag: bool) -> i64 {\n",
            "    match flag { true => 1i64, false => 2i64 }\n",
            "}\n",
            "flow start() -> i64 { return main(true) }\n",
            "pub view Mobile(dialogue: DialogueView) {\n",
            "    RichText(dialogue.content)\n",
            "}\n",
            "pub style Mobile {\n",
            "    Button { color = rgba(10, 20, 30, 255) }\n",
            "}\n",
            "fn tail(value: i64) -> i64 { value }\n",
        ),
        "runtime-exact-types",
    );
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("typed Match project compiles");
    let executable = compiled
        .hir_project()
        .executable_view()
        .expect("accepted project is executable");
    let runtime_owners = project_runtime_reachability(
        executable,
        compiled.project_symbols(),
        compiled.final_analysis(),
        compiled.checked_entries(),
        RuntimeEmissionMode::CheckAll,
    )
    .expect("accepted runtime reachability");
    let runtime_facts = project_runtime_semantic_facts(
        executable,
        compiled.project_symbols(),
        compiled.registered_world(),
        compiled.final_analysis(),
        &runtime_owners,
        None,
        None,
    )
    .expect("accepted types project through the compiler boundary");
    let expression_type_owners = runtime_owners
        .selected_expression_type_owners()
        .expect("accepted runtime expression type owners");

    let mut saw_bool_local = false;
    let mut saw_presentation_local = false;
    for (owner, checked) in compiled.final_analysis().locals() {
        if !runtime_owners.contains_local(owner) {
            saw_presentation_local = true;
            assert!(runtime_facts.local_type(owner).is_none());
            continue;
        }
        let projected = runtime_facts
            .local_type(owner)
            .expect("every runtime-domain local retains one runtime type fact");
        assert_eq!(
            projected.identity(),
            RuntimeSemanticTypeId::from_bytes(*checked.ty().semantic_identity_digest().as_bytes())
        );
        if matches!(
            (checked.ty(), projected.shape()),
            (TypeKind::Bool, RuntimeTypeShape::Bool)
        ) {
            saw_bool_local = true;
        }
    }

    let mut saw_bool_expression = false;
    let mut saw_i64_expression = false;
    let mut saw_presentation_expression = false;
    for (owner, checked) in compiled.final_analysis().expressions() {
        if !expression_type_owners.contains(&owner) {
            saw_presentation_expression |= !runtime_owners.contains_expression(owner);
            assert!(runtime_facts.expression_type(owner).is_none());
            continue;
        }
        let projected = runtime_facts
            .expression_type(owner)
            .expect("every selected runtime expression retains one runtime type fact");
        assert_eq!(
            projected.identity(),
            RuntimeSemanticTypeId::from_bytes(*checked.ty().semantic_identity_digest().as_bytes())
        );
        match (checked.ty(), projected.shape()) {
            (TypeKind::Bool, RuntimeTypeShape::Bool) => saw_bool_expression = true,
            (TypeKind::I64, RuntimeTypeShape::Signed(_)) => saw_i64_expression = true,
            _ => {}
        }
    }

    let mut saw_bool_pattern = false;
    let mut saw_presentation_pattern = false;
    for (owner, checked) in compiled.final_analysis().patterns() {
        if !runtime_owners.contains_pattern(owner) {
            saw_presentation_pattern = true;
            assert!(runtime_facts.pattern_type(owner).is_none());
            continue;
        }
        let projected = runtime_facts
            .pattern_type(owner)
            .expect("every runtime-domain pattern retains one runtime type fact");
        assert_eq!(
            projected.identity(),
            RuntimeSemanticTypeId::from_bytes(*checked.ty().semantic_identity_digest().as_bytes())
        );
        if matches!(
            (checked.ty(), projected.shape()),
            (TypeKind::Bool, RuntimeTypeShape::Bool)
        ) {
            saw_bool_pattern = true;
        }
    }

    let mut saw_presentation_type = false;
    for (owner, checked) in compiled.final_analysis().types() {
        if !runtime_owners.contains_type(owner) {
            saw_presentation_type = true;
            assert!(runtime_facts.ty(owner).is_none());
            continue;
        }
        let projected = runtime_facts
            .ty(owner)
            .expect("every runtime-domain authored type retains one runtime type fact");
        assert_eq!(
            projected.identity(),
            RuntimeSemanticTypeId::from_bytes(*checked.semantic_identity_digest().as_bytes())
        );
    }

    assert!(saw_bool_local, "Bool local projection is present");
    assert!(saw_bool_expression, "Bool expression projection is present");
    assert!(saw_i64_expression, "i64 expression projection is present");
    assert!(saw_bool_pattern, "Bool pattern projection is present");
    assert!(saw_presentation_local, "View local projection is absent");
    assert!(
        saw_presentation_expression,
        "View/Style expression projection is absent"
    );
    assert!(
        saw_presentation_pattern,
        "View pattern projection is absent"
    );
    assert!(
        saw_presentation_type,
        "View/Style type projection is absent"
    );
}

#[test]
fn unreachable_assignment_retains_checked_place_but_publishes_no_runtime_fact() {
    let (project, facts) = fixture(
        concat!(
            "struct Point { x: i64, active: bool }\n",
            "fn main(point: Point) -> bool {\n",
            "    point.active = true\n",
            "    point.active\n",
            "}\n",
        ),
        "runtime-assignment-fact",
    );
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("direct record-field assignment compiles");
    let executable = compiled
        .hir_project()
        .executable_view()
        .expect("accepted assignment project is executable");
    let statement = executable
        .modules()
        .flat_map(|(_, module)| module.statements())
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Assign { .. }).then_some(owner)
        })
        .expect("assignment statement");
    let checked = compiled
        .final_analysis()
        .statement(statement)
        .expect("checked assignment statement");
    let CheckedStatementPayload::Assignment(checked) = checked.payload() else {
        panic!("assignment owns its checked semantic role")
    };
    let runtime_owners = project_runtime_reachability(
        executable,
        compiled.project_symbols(),
        compiled.final_analysis(),
        compiled.checked_entries(),
        RuntimeEmissionMode::CheckAll,
    )
    .expect("accepted assignment reachability");
    let runtime = project_runtime_semantic_facts(
        executable,
        compiled.project_symbols(),
        compiled.registered_world(),
        compiled.final_analysis(),
        &runtime_owners,
        None,
        None,
    )
    .expect("assignment projects through the compiler boundary");
    assert!(runtime.assignment(statement).is_none());
    assert!(!runtime_owners.contains_statement(statement));
    assert_eq!(checked.place().field_type(), &TypeKind::Bool);
    assert_eq!(checked.value_type(), &TypeKind::Bool);
}

#[test]
fn runtime_variant_facts_retain_the_complete_normalized_project_case_table() {
    let (project, facts) = fixture(
        "enum Event {\n    Unit,\n    Text String,\n}\n\nfn event() -> Event { .Unit }\n\nflow main() -> String { let current = event(); return \"done\" }\n",
        "runtime-normalized-variant-cases",
    );
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("project enum fixture compiles");
    let executable = compiled
        .hir_project()
        .executable_view()
        .expect("accepted project is executable");
    let runtime_owners = project_runtime_reachability(
        executable,
        compiled.project_symbols(),
        compiled.final_analysis(),
        compiled.checked_entries(),
        RuntimeEmissionMode::CheckAll,
    )
    .expect("accepted project enum reachability");
    let runtime_facts = project_runtime_semantic_facts(
        executable,
        compiled.project_symbols(),
        compiled.registered_world(),
        compiled.final_analysis(),
        &runtime_owners,
        None,
        None,
    )
    .expect("project enum cases project from accepted semantic types");
    let selected = executable
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .filter_map(|(owner, _)| runtime_facts.expression_variant(owner))
        .find(|variant| variant.selected_name() == Ok("Unit"))
        .expect("unit expression retains its selected project variant fact");

    assert_eq!(selected.selected_payload_type(), Ok(None));
    let RuntimeVariantOwner::Project { cases, .. } = selected.owner() else {
        panic!("project enum expression retains a project variant owner");
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].name(), "Unit");
    assert!(cases[0].payload().is_none());
    assert_eq!(cases[1].name(), "Text");
    let payload = cases[1]
        .payload()
        .expect("Text case retains its canonical payload type");
    let RuntimeTypeShape::Tuple(items) = payload.shape() else {
        panic!("Text case payload is a canonical one-field tuple");
    };
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0].shape(), RuntimeTypeShape::String));
}

#[test]
fn lowered_hir_cache_rejects_exact_source_from_another_syntax_and_hir_session() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "cross-session-miss");
    let root = CanonicalModulePath::crate_root();
    let mut cache = RecordingCache::default();
    let mut first_compiler = AttachedCompiler::new(&project);
    let first_syntax_database = first_compiler.parsed_sources[&root]
        .snapshot_id()
        .lineage()
        .database();
    let first_hir_database = first_compiler.session.hir_database_id();
    let first = first_compiler
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), Arc::clone(&facts)),
            &mut cache,
        )
        .expect("first session compiles");
    cache.reset_activity();

    let mut second_compiler = AttachedCompiler::new(&project);
    let second_syntax_database = second_compiler.parsed_sources[&root]
        .snapshot_id()
        .lineage()
        .database();
    let second_hir_database = second_compiler.session.hir_database_id();
    assert_ne!(first_syntax_database, second_syntax_database);
    assert_ne!(first_hir_database, second_hir_database);

    let second = second_compiler
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("foreign in-memory HIR is rebuilt in the second session");

    assert_eq!(cache.loads, 1);
    assert_eq!(cache.stores, 1);
    assert_eq!(
        first.compile_units()[0].fingerprint(),
        second.compile_units()[0].fingerprint()
    );
    assert_eq!(
        second.compile_units()[0].cache_status(),
        ProjectCompileCacheStatus::Miss
    );
    assert_eq!(
        second.modules()[0].hir().snapshot_id().module().database(),
        second_hir_database
    );
}

#[test]
fn lowered_hir_cache_hit_remains_read_only() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "read-only-hit");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let first = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), Arc::clone(&facts)),
            &mut cache,
        )
        .expect("first compilation");
    assert_eq!(cache.stores, 1);
    cache.reset_activity();

    let hit = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("cache-hit compilation still registers and typechecks");

    assert_eq!(cache.loads, 1);
    assert_eq!(cache.stores, 0);
    assert_eq!(
        hit.compile_units()[0].cache_status(),
        ProjectCompileCacheStatus::Hit
    );
    assert_eq!(
        hit.registered_environment().character_digest(),
        first.registered_environment().character_digest()
    );
    hit.registered_environment()
        .verify_character_inventory(hit.project_symbols())
        .expect("hit path produced a complete registered world");
}

#[test]
fn lowered_hir_cache_rejects_another_document_identity_with_the_same_bytes() {
    let source = "fn main() -> Unit { () }\n";
    let profile = "identity-miss";
    let (first_project, first_document, first_world) = project_fixture_with_document_id(
        source,
        profile,
        "arcweft-project://identity-first/src/main.arcw",
    );
    let first_facts = registration_facts(first_document, first_world);
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&first_project);
    let first = session
        .compile(
            &first_project,
            &context(TypeCheckEnv::standard(), first_facts),
            &mut cache,
        )
        .expect("first compilation");
    cache.reset_activity();

    let (second_project, second_document, second_world) = project_fixture_with_document_id(
        source,
        profile,
        "arcweft-project://identity-second/src/main.arcw",
    );
    let expected_identity = second_document.identity().clone();
    let second_facts = registration_facts(second_document, second_world);
    session.replace_sources(&second_project);
    let second = session
        .compile(
            &second_project,
            &context(TypeCheckEnv::standard(), second_facts),
            &mut cache,
        )
        .expect("second compilation");

    assert_eq!(cache.loads, 1, "the identical content key was consulted");
    assert_eq!(
        cache.stores, 1,
        "the foreign document artifact was replaced"
    );
    assert_eq!(
        first.compile_units()[0].fingerprint(),
        second.compile_units()[0].fingerprint()
    );
    assert_eq!(
        second.compile_units()[0].cache_status(),
        ProjectCompileCacheStatus::Miss
    );
    assert_eq!(second.modules()[0].source(), &expected_identity);
}

#[test]
fn source_revision_change_invalidates_the_compile_unit_cache() {
    let profile = "revision-miss";
    let document_id = "arcweft-project://revision-miss/src/main.arcw";
    let (first_project, first_document, first_world) =
        project_fixture_with_document_id("fn main() -> Unit { () }\n", profile, document_id);
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&first_project);
    let first = session
        .compile(
            &first_project,
            &context(
                TypeCheckEnv::standard(),
                registration_facts(first_document, first_world),
            ),
            &mut cache,
        )
        .expect("first compilation");
    cache.reset_activity();

    let (changed_project, changed_document, changed_world) =
        project_fixture_with_document_id("fn main() -> Unit {\n    ()\n}\n", profile, document_id);
    let changed_revision = changed_document.identity().revision();
    session.replace_sources(&changed_project);
    let changed = session
        .compile(
            &changed_project,
            &context(
                TypeCheckEnv::standard(),
                registration_facts(changed_document, changed_world),
            ),
            &mut cache,
        )
        .expect("changed compilation");

    assert_eq!(cache.loads, 1);
    assert_eq!(cache.stores, 1);
    assert_ne!(
        first.compile_units()[0].fingerprint(),
        changed.compile_units()[0].fingerprint()
    );
    assert_eq!(
        changed.compile_units()[0].cache_status(),
        ProjectCompileCacheStatus::Miss
    );
    assert_eq!(changed.modules()[0].source().revision(), changed_revision);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cache invalidation transaction must assert changed, dependent, and retained modules together"
)]
fn symbol_table_revision_invalidates_exact_changed_modules() {
    fn unit<'project>(
        project: &'project CompiledProject,
        module: &CanonicalModulePath,
    ) -> &'project arcweft_compiler::project::ProjectCompileUnitSummary {
        project
            .compile_units()
            .iter()
            .find(|unit| unit.modules() == std::slice::from_ref(module))
            .unwrap_or_else(|| panic!("one-module compile unit for {module}"))
    }

    fn compiled_module<'project>(
        project: &'project CompiledProject,
        module: &CanonicalModulePath,
    ) -> &'project CompiledProjectModule {
        project
            .modules()
            .iter()
            .find(|compiled| compiled.module() == module)
            .unwrap_or_else(|| panic!("compiled module for {module}"))
    }

    let profile = "three-unit-symbol-invalidation";
    let root = three_unit_document(
        profile,
        "main",
        "use crate.dependency.value\nfn main() -> i32 { value() }\n",
    );
    let first_dependency =
        three_unit_document(profile, "dependency", "pub fn value() -> i32 { 1 }\n");
    let unrelated = three_unit_document(profile, "unrelated", "pub fn steady() -> i32 { 7 }\n");
    let (first_sources, first_facts) = three_unit_project(
        profile,
        Arc::clone(&root),
        first_dependency,
        Arc::clone(&unrelated),
    );
    let mut session = AttachedCompiler::new(&first_sources);
    let mut cache = RecordingCache::default();
    let first = session
        .compile(
            &first_sources,
            &context(TypeCheckEnv::standard(), first_facts),
            &mut cache,
        )
        .expect("initial three-unit project compiles");
    assert_eq!(first.compile_units().len(), 3);
    assert!(
        first
            .compile_units()
            .iter()
            .all(|unit| unit.cache_status() == ProjectCompileCacheStatus::Miss)
    );
    cache.reset_activity();

    let changed_dependency = three_unit_document(
        profile,
        "dependency",
        "pub(crate) fn value() -> i32 { 1 }\n",
    );
    let (changed_sources, changed_facts) = three_unit_project(
        profile,
        Arc::clone(&root),
        changed_dependency,
        Arc::clone(&unrelated),
    );
    session.replace_sources(&changed_sources);
    let changed = session
        .compile(
            &changed_sources,
            &context(TypeCheckEnv::standard(), changed_facts),
            &mut cache,
        )
        .expect("changed dependency publishes only after every required rebuild succeeds");

    let root_path = CanonicalModulePath::crate_root();
    let dependency_path = child_module("dependency");
    let unrelated_path = child_module("unrelated");
    let first_root = unit(&first, &root_path);
    let first_dependency = unit(&first, &dependency_path);
    let first_unrelated = unit(&first, &unrelated_path);
    let changed_root = unit(&changed, &root_path);
    let changed_dependency = unit(&changed, &dependency_path);
    let changed_unrelated = unit(&changed, &unrelated_path);

    assert_ne!(
        first_dependency.fingerprint(),
        changed_dependency.fingerprint()
    );
    assert_ne!(first_root.fingerprint(), changed_root.fingerprint());
    assert_eq!(
        first_unrelated.fingerprint(),
        changed_unrelated.fingerprint()
    );
    assert_eq!(
        changed_dependency.cache_status(),
        ProjectCompileCacheStatus::Miss
    );
    assert_eq!(changed_root.cache_status(), ProjectCompileCacheStatus::Miss);
    assert_eq!(
        changed_unrelated.cache_status(),
        ProjectCompileCacheStatus::Hit
    );
    assert!(
        Arc::ptr_eq(
            compiled_module(&first, &unrelated_path).hir(),
            compiled_module(&changed, &unrelated_path).hir(),
        ),
        "a cache hit retains the exact accepted HIR module lease",
    );
    assert_eq!(cache.loads, 3, "every unit consults its typed fingerprint");
    assert_eq!(cache.stores, 2, "only changed and dependent units rebuild");
    assert_ne!(
        first.project_symbols().revision(),
        changed.project_symbols().revision()
    );
    assert_eq!(
        changed.project_symbols().revision(),
        changed.registered_environment().symbol_revision(),
        "symbol publication and registered semantic facts share the final project revision",
    );
    changed
        .registered_environment()
        .verify_character_inventory(changed.project_symbols())
        .expect("no partial registration is observable after the two required rebuilds");
}

#[test]
fn pending_stores_flush_after_complete_success() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "flush-success");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);

    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("complete project compilation");

    assert_eq!(cache.loads, 1);
    assert_eq!(cache.stores, 1);
    assert_eq!(cache.units.len(), 1);
    assert_eq!(
        compiled.compile_units()[0].cache_status(),
        ProjectCompileCacheStatus::Miss
    );
    compiled
        .registered_environment()
        .verify_character_inventory(compiled.project_symbols())
        .expect("cache stores flush only for a complete registered project");
}

#[test]
fn pending_stores_discard_on_type_error() {
    let (project, facts) = fixture("fn main() -> i32 { true }\n", "discard-type-error");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);

    let error = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect_err("return type mismatch rejects compilation");

    assert_eq!(error.stage(), "type-check");
    assert_eq!(cache.stores, 0);
    assert!(cache.units.is_empty());
}

#[test]
fn character_digest_cannot_key_semantic_reuse() {
    let (project, facts) = fixture("fn main() -> i32 { configured }\n", "semantic-reuse");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let first_base = TypeCheckEnv::standard().with_symbol("configured", TypeKind::I32);
    let first = session
        .compile(
            &project,
            &context(first_base, Arc::clone(&facts)),
            &mut cache,
        )
        .expect("first semantic world");
    let changed_base = TypeCheckEnv::standard().with_symbol("configured", TypeKind::Bool);
    let changed_registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(changed_base.clone()),
        first.hir_project().view(),
        &facts,
        Some(first.registered_environment()),
    ))
    .expect("registration remains valid under a base-only change");
    assert_eq!(
        changed_registered.environment().character_digest(),
        first.registered_environment().character_digest()
    );
    cache.reset_activity();

    let error = session
        .compile(&project, &context(changed_base, facts), &mut cache)
        .expect_err("base change must rerun semantic checking after a HIR hit");

    assert_eq!(cache.loads, 1, "the lowered HIR cache was consulted");
    assert_eq!(cache.stores, 0, "a hit is read-only even on later failure");
    assert_eq!(error.stage(), "type-check");
}

#[test]
fn compiled_project_holds_one_registered_world() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "one-world");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("compiled project");

    assert_eq!(
        compiled.project_symbols().world(),
        compiled.registered_environment().world()
    );
    assert_eq!(
        compiled.project_symbols().revision(),
        compiled.registered_environment().symbol_revision()
    );
    assert!(std::ptr::eq(
        compiled.project_symbols(),
        compiled.registered_world().symbols()
    ));
    assert!(std::ptr::eq(
        compiled.registered_environment(),
        compiled.registered_world().environment()
    ));
}

#[test]
fn compiled_snapshot_carries_the_registered_environment_digest() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "environment-digest");
    let mut cache = RecordingCache::default();
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(TypeCheckEnv::standard(), facts),
            &mut cache,
        )
        .expect("compiled project");
    let expected = BuildDigest::from_bytes(
        *compiled
            .registered_environment()
            .environment_digest()
            .as_bytes(),
    );

    let snapshot = snapshot_compiled_project(
        &project,
        &compiled,
        BuildSnapshotRequest {
            build_id: "environment-digest-build".to_owned(),
            compiler_build_id: "compiler-test".to_owned(),
            target_triple: "test-target".to_owned(),
            target_features: Vec::new(),
            profile: "test".to_owned(),
            selected_entries: Vec::new(),
        },
    );

    assert_ne!(expected, BuildDigest::ZERO);
    assert_eq!(snapshot.project().adapter_environment_digest(), expected);
}

#[test]
fn identical_accepted_environment_reuses_the_persistent_query_key() {
    let manifest = nominal_manifest("persistent-fixture", "Rank", false);
    let first = snapshot_with_manifest("stable-environment", &manifest);
    let second = snapshot_with_manifest("stable-environment", &manifest);

    assert_eq!(first.project(), second.project());
    assert_eq!(first.queries().len(), second.queries().len());
    assert!(
        first
            .queries()
            .iter()
            .zip(second.queries())
            .all(|(left, right)| left.key() == right.key()),
        "an identical accepted semantic environment must reproduce every persistent query key"
    );
}

#[test]
fn accepted_nominal_owner_and_path_invalidate_persistent_query_keys() {
    let baseline = snapshot_with_manifest(
        "nominal-identity-change",
        &nominal_manifest("persistent-fixture", "Rank", false),
    );
    let changed_path = snapshot_with_manifest(
        "nominal-identity-change",
        &nominal_manifest("persistent-fixture", "Standing", false),
    );
    let changed_owner = snapshot_with_manifest(
        "nominal-identity-change",
        &nominal_manifest("other-persistent-fixture", "Rank", false),
    );

    let baseline_digest = baseline.project().adapter_environment_digest();
    assert_ne!(
        baseline_digest,
        changed_path.project().adapter_environment_digest()
    );
    assert_ne!(
        baseline_digest,
        changed_owner.project().adapter_environment_digest()
    );
    assert_ne!(baseline.queries()[0].key(), changed_path.queries()[0].key());
    assert_ne!(
        baseline.queries()[0].key(),
        changed_owner.queries()[0].key()
    );
}

#[test]
fn nested_callable_type_change_invalidates_persistent_query_keys() {
    let direct = snapshot_with_manifest(
        "nested-callable-type-change",
        &nominal_manifest("persistent-fixture", "Rank", false),
    );
    let optional = snapshot_with_manifest(
        "nested-callable-type-change",
        &nominal_manifest("persistent-fixture", "Rank", true),
    );

    assert_ne!(
        direct.project().adapter_environment_digest(),
        optional.project().adapter_environment_digest()
    );
    assert_ne!(direct.queries()[0].key(), optional.queries()[0].key());
}
