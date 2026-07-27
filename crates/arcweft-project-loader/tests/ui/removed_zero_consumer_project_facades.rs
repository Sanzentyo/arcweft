use arcweft_project_loader::character_manifest::{
    LoadedCharacterManifest, decode, load, load_for_project, manifest_path,
};
use arcweft_project_loader::environment::{
    LoadedProjectRegistration, ProjectRegistrationLoadError,
};
use arcweft_project_loader::project::{LoadedProject, load_discovered_with_limits};

fn removed_registration_projections(registration: &LoadedProjectRegistration) {
    let _ = registration.facts();
    let _ = registration.file_documents();
}

fn removed_character_manifest_error_variant() {
    let _ = ProjectRegistrationLoadError::CharacterManifest;
}

fn removed_consuming_projection(project: LoadedProject) {
    let _ = project.into_sources();
}

fn main() {
    let _ = (
        LoadedCharacterManifest,
        decode,
        load,
        load_for_project,
        manifest_path,
    );
    let _ = load_discovered_with_limits;
}
