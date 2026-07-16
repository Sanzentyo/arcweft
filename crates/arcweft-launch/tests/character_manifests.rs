use arcweft_launch::LaunchProfileManifest;
use std::path::{Path, PathBuf};

#[test]
fn resolves_character_manifests_relative_to_project_root() {
    let manifest = LaunchProfileManifest::parse_toml(
        r#"
[profiles.game]
kind = "game"
source = "src/main.arcw"
entry = "entry.game"
character_manifests = ["assets/akane.awchar", "assets/sub/alice.arcwchar.json"]
"#,
    )
    .expect("manifest parses");

    let profile = manifest
        .resolve_profile("game", Path::new("project"))
        .expect("profile resolves");

    assert_eq!(
        profile.character_manifests(),
        &[
            PathBuf::from("project/assets/akane.awchar"),
            PathBuf::from("project/assets/sub/alice.arcwchar.json"),
        ]
    );
}
