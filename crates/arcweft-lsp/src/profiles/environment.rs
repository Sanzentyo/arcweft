use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, RegisteredSemanticWorld,
        RegisteredTypeCheckEnv,
    },
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_launch::ResolvedLaunchProfile;
use std::{path::Path, sync::Arc};

pub(super) fn register_profile_environment(
    manifest_path: &Path,
    profile: &ResolvedLaunchProfile,
    adapter: &AdapterManifest,
    base: TypeCheckEnv,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<Arc<RegisteredSemanticWorld>, String> {
    let loaded = arcweft_project_loader::project::load(manifest_path)
        .map_err(|error| format!("failed to load registration project: {error}"))?;
    let modules = loaded
        .sources()
        .modules()
        .map(|source| {
            let document = loaded
                .module_document(source.module())
                .expect("loaded project retains one document per source module");
            let parsed = parse_source(document.text());
            if !parsed.errors().is_empty() {
                return Err(format!(
                    "source `{}` has syntax errors: {:?}",
                    source.path().display(),
                    parsed.errors()
                ));
            }
            let hir = lower_document_to_hir(document, parsed.typed_tree()).map_err(|errors| {
                format!(
                    "HIR lowering failed for `{}`: {errors:?}",
                    source.path().display()
                )
            })?;
            Ok(HirProjectModule::new(
                source.module().clone(),
                document.identity().clone(),
                hir,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let project = HirProject::new(
        loaded.sources().manifest().package().name().as_str(),
        modules,
    )
    .map_err(|error| format!("failed to assemble registration project: {error}"))?;
    register_loaded_environment(
        &loaded,
        &project,
        Some(profile),
        std::slice::from_ref(adapter),
        base,
        Vec::new(),
        previous,
    )
}

pub(crate) fn register_loaded_environment(
    loaded: &arcweft_project_loader::project::LoadedProject,
    project: &HirProject,
    profile: Option<&ResolvedLaunchProfile>,
    adapter_manifests: &[AdapterManifest],
    base: TypeCheckEnv,
    additional_documents: Vec<Arc<arcweft_source::SourceDocument>>,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<Arc<RegisteredSemanticWorld>, String> {
    let request = arcweft_project_loader::environment::ProjectLoadRequest::new(
        loaded,
        profile,
        additional_documents,
        Vec::new(),
    )
    .with_adapter_manifests(adapter_manifests.iter().cloned());
    let facts = arcweft_project_loader::environment::load_project_registration_facts(&request)
        .map_err(|error| format!("failed to load registration facts: {error}"))?;
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        project,
        &facts,
        previous,
    ))
    .map(Arc::new)
    .map_err(|report| {
        let details = report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.diagnostic().message().to_owned())
            .collect::<Vec<_>>()
            .join("; ");
        format!("project registration was rejected: {details}")
    })
}
