use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_compiler::project::{
    CompiledProjectModule, ProjectCompilationContext, ProjectCompileCache,
    ProjectCompileCacheStatus, ProjectCompileUnitFingerprint, compile_project_with_cache,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
    types::TypeKind,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_project::{
    manifest::ProjectManifest,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

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

fn fixture(source: &str, profile: &str) -> (ProjectSources, Arc<ProjectRegistrationFacts>) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-project://compiler-cache-{profile}/src/main.arcw"
            ))
            .expect("document id"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("document"),
    );
    let manifest =
        ProjectManifest::parse_toml(&format!("[package]\nname = \"compiler-cache-{profile}\"\n"))
            .expect("manifest");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        manifest,
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(format!("compiler-cache-{profile}")).expect("package"),
        document.identity().id().clone(),
        profile,
    )
    .expect("world");
    let facts = Arc::new(
        ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
            .expect("registration facts"),
    );
    (project, facts)
}

fn context(base: TypeCheckEnv, facts: Arc<ProjectRegistrationFacts>) -> ProjectCompilationContext {
    ProjectCompilationContext::new(Arc::new(base), facts, None)
}

#[test]
fn lowered_hir_cache_hit_remains_read_only() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "read-only-hit");
    let mut cache = RecordingCache::default();
    let first = compile_project_with_cache(
        &project,
        &context(TypeCheckEnv::standard(), Arc::clone(&facts)),
        &RuntimePlanLowerOptions::default(),
        &mut cache,
    )
    .expect("first compilation");
    assert_eq!(cache.stores, 1);
    cache.reset_activity();

    let hit = compile_project_with_cache(
        &project,
        &context(TypeCheckEnv::standard(), facts),
        &RuntimePlanLowerOptions::default(),
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
fn pending_stores_flush_after_complete_success() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "flush-success");
    let mut cache = RecordingCache::default();

    let compiled = compile_project_with_cache(
        &project,
        &context(TypeCheckEnv::standard(), facts),
        &RuntimePlanLowerOptions::default(),
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

    let error = compile_project_with_cache(
        &project,
        &context(TypeCheckEnv::standard(), facts),
        &RuntimePlanLowerOptions::default(),
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
    let first_base = TypeCheckEnv::standard().with_symbol("configured", TypeKind::I32);
    let first = compile_project_with_cache(
        &project,
        &context(first_base, Arc::clone(&facts)),
        &RuntimePlanLowerOptions::default(),
        &mut cache,
    )
    .expect("first semantic world");
    let changed_base = TypeCheckEnv::standard().with_symbol("configured", TypeKind::Bool);
    let changed_registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(changed_base.clone()),
        first.hir_project(),
        &facts,
        Some(first.registered_environment()),
    ))
    .expect("registration remains valid under a base-only change");
    assert_eq!(
        changed_registered.environment().character_digest(),
        first.registered_environment().character_digest()
    );
    cache.reset_activity();

    let error = compile_project_with_cache(
        &project,
        &context(changed_base, facts),
        &RuntimePlanLowerOptions::default(),
        &mut cache,
    )
    .expect_err("base change must rerun semantic checking after a HIR hit");

    assert_eq!(cache.loads, 1, "the lowered HIR cache was consulted");
    assert_eq!(cache.stores, 0, "a hit is read-only even on later failure");
    assert_eq!(error.stage(), "type-check");
}

#[test]
fn compiled_project_holds_one_registered_world() {
    let (project, facts) = fixture("fn main() -> Unit { () }\n", "one-world");
    let mut cache = RecordingCache::default();
    let compiled = compile_project_with_cache(
        &project,
        &context(TypeCheckEnv::standard(), facts),
        &RuntimePlanLowerOptions::default(),
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
