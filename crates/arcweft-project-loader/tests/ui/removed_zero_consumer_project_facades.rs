use arcweft_project_loader::project::{LoadedProject, load_discovered_with_limits};

fn removed_consuming_projection(project: LoadedProject) {
    let _ = project.into_sources();
}

fn main() {
    let _ = load_discovered_with_limits;
}
