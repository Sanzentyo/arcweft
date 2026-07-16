use super::*;
use std::path::{Path, PathBuf};

fn selection_manifest(default: Option<&str>) -> LaunchProfileManifest {
    let default = default.map_or(String::new(), |profile| {
        format!("default = \"{profile}\"\n")
    });
    LaunchProfileManifest::parse_toml(&format!(
        r#"{default}
[profiles.alpha]
kind = "game"
source = "alpha.arcw"

[profiles.beta]
kind = "game"
source = "beta.arcw"
"#
    ))
    .expect("selection manifest parses")
}

#[test]
fn explicit_profile_selection_is_exact() {
    let manifest = selection_manifest(Some("alpha"));
    assert_eq!(
        manifest
            .select_profile_id(LaunchProfileSelection::Explicit("beta"))
            .expect("explicit profile exists"),
        "beta"
    );
    assert!(matches!(
        manifest.select_profile_id(LaunchProfileSelection::Explicit("missing")),
        Err(LaunchProfileError::MissingProfile(profile)) if profile == "missing"
    ));
}

#[test]
fn automatic_selection_prefers_valid_manifest_default() {
    let manifest = selection_manifest(Some("alpha"));
    assert_eq!(
        manifest
            .select_profile_id(LaunchProfileSelection::Automatic {
                previous: Some("beta"),
            })
            .expect("default profile exists"),
        "alpha"
    );
}

#[test]
fn automatic_selection_rejects_missing_manifest_default() {
    let manifest = selection_manifest(Some("missing"));
    assert!(matches!(
        manifest.select_profile_id(LaunchProfileSelection::Automatic {
            previous: Some("beta"),
        }),
        Err(LaunchProfileError::InvalidDefaultProfile { profile }) if profile == "missing"
    ));
}

#[test]
fn automatic_selection_retains_existing_previous_without_default() {
    let manifest = selection_manifest(None);
    assert_eq!(
        manifest
            .select_profile_id(LaunchProfileSelection::Automatic {
                previous: Some("beta"),
            })
            .expect("previous profile exists"),
        "beta"
    );
}

#[test]
fn automatic_selection_falls_back_to_lexicographic_first() {
    let manifest = selection_manifest(None);
    assert_eq!(
        manifest
            .select_profile_id(LaunchProfileSelection::Automatic {
                previous: Some("removed"),
            })
            .expect("lexical fallback exists"),
        "alpha"
    );
}

#[test]
fn automatic_selection_rejects_empty_profile_map() {
    let manifest = LaunchProfileManifest::parse_toml("").expect("empty manifest parses");
    assert!(matches!(
        manifest.select_profile_id(LaunchProfileSelection::Automatic { previous: None }),
        Err(LaunchProfileError::NoProfiles)
    ));
}

#[test]
fn parses_and_resolves_profiles_relative_to_manifest_dir() {
    let manifest = LaunchProfileManifest::parse_toml(
        r#"
default = "server.dev"

[profiles."server.dev"]
kind = "server"
source = "src/server.arcw"
entry = "http"
adapter = "native-http"
adapter_manifests = ["adapters/http.toml"]
listen = "127.0.0.1:8787"
dialogue_defaults = "dialogue.mobile"
rust_metadata = ["target/arcweft/truck_game.json"]

[profiles."server.dev".pure]
backend = "jit"
math_backend = "ndarray"
math_wgpu_min_elements = 1024
workers = "auto"
batch_min_len = 2048
object_artifacts = true
"#,
    )
    .expect("manifest parses");

    assert_eq!(manifest.default_profile(), Some("server.dev"));
    assert_eq!(LaunchKind::Server.as_str(), "server");
    assert_eq!(
        manifest
            .profiles()
            .get("server.dev")
            .map(LaunchProfileSpec::kind),
        Some(LaunchKind::Server)
    );
    assert_eq!(
        manifest.profile_ids_with_kind(LaunchKind::Server),
        vec!["server.dev".to_owned()]
    );

    let resolved = manifest
        .resolve_profile_with_adapters("server.dev", Path::new("game"), &["native-http"])
        .expect("profile resolves");

    assert_eq!(resolved.kind(), LaunchKind::Server);
    assert_eq!(resolved.source(), Path::new("game/src/server.arcw"));
    assert_eq!(resolved.entry(), Some("http"));
    assert_eq!(resolved.adapter(), Some("native-http"));
    assert_eq!(
        resolved.adapter_manifests(),
        &[PathBuf::from("game/adapters/http.toml")]
    );
    assert_eq!(resolved.listen(), Some("127.0.0.1:8787"));
    assert_eq!(resolved.dialogue_defaults(), Some("dialogue.mobile"));
    assert_eq!(
        resolved.rust_metadata(),
        &[PathBuf::from("game/target/arcweft/truck_game.json")]
    );
    let pure = resolved.pure().expect("pure profile resolves");
    assert_eq!(pure.backend(), Some(LaunchPureBackend::Jit));
    assert_eq!(pure.math_backend(), Some(LaunchMathBackend::Ndarray));
    assert_eq!(pure.math_wgpu_min_elements(), Some(1024));
    assert_eq!(pure.workers(), Some("auto"));
    assert_eq!(pure.batch_min_len(), Some(2048));
    assert_eq!(pure.object_artifacts(), Some(true));
}

#[test]
fn parses_build_hot_reload_and_content_profile_policy() {
    let manifest = LaunchProfileManifest::parse_toml(
        r#"
[profiles.release]
kind = "game"
source = "src/main.arcw"
entry = "entry.main"

[profiles.release.build]
incremental = true
tree_shake = true
debug = "line-tables"
source = "none"
shared_hoist_threshold_bytes = 65536

[profiles.release.hot_reload]
mode = "swap"
fallback = "restart"
state = "strict"

[profiles.release.player.viewport]
design-width = 1280
design-height = 720
fit = "contain"

[profiles.release.content."content.chapter_two"]
residency = "on-demand"
placement = "external"
compression = "zstd"

[profiles.desktop]
kind = "game"
source = "src/main.arcw"

[profiles.desktop.content."content.chapter_two"]
residency = "on-demand"
placement = "embedded"
"#,
    )
    .expect("manifest parses");

    let release = manifest
        .resolve_profile("release", Path::new("game"))
        .expect("release profile resolves");
    let build = release.build();
    assert_eq!(build.incremental(), Some(true));
    assert_eq!(build.tree_shake(), Some(true));
    assert_eq!(build.debug(), Some(LaunchDebugPolicy::LineTables));
    assert_eq!(build.source(), Some(LaunchSourcePolicy::None));
    assert_eq!(build.shared_hoist_threshold_bytes(), Some(65_536));
    assert_eq!(LaunchDebugPolicy::LineTables.to_string(), "line-tables");
    assert_eq!(LaunchSourcePolicy::None.to_string(), "none");

    let hot_reload = release.hot_reload().expect("hot reload policy");
    assert_eq!(hot_reload.mode(), Some(LaunchHotReloadMode::Swap));
    assert_eq!(
        hot_reload.fallback(),
        Some(LaunchHotReloadFallback::Restart)
    );
    assert_eq!(hot_reload.state(), Some(LaunchHotReloadStatePolicy::Strict));
    let viewport = release.player().viewport().expect("player viewport policy");
    assert_eq!(viewport.design_width(), 1280);
    assert_eq!(viewport.design_height(), 720);
    assert_eq!(viewport.fit(), LaunchPlayerViewportFit::Contain);

    let content = release
        .content()
        .get("content.chapter_two")
        .expect("content policy");
    assert_eq!(content.residency(), ContentResidency::OnDemand);
    assert_eq!(content.placement(), ContentPlacement::External);
    assert_eq!(content.compression(), ContentCompression::Zstd);
    assert_eq!(ContentResidency::OnDemand.to_string(), "on-demand");
    assert_eq!(ContentPlacement::External.to_string(), "external");
    assert_eq!(ContentCompression::Zstd.to_string(), "zstd");

    let desktop = manifest
        .resolve_profile("desktop", Path::new("game"))
        .expect("desktop profile resolves");
    let desktop_content = desktop
        .content()
        .get("content.chapter_two")
        .expect("desktop content policy");
    assert_eq!(desktop_content.residency(), ContentResidency::OnDemand);
    assert_eq!(desktop_content.placement(), ContentPlacement::Embedded);
    assert_eq!(desktop_content.compression(), ContentCompression::None);
}

#[test]
fn rejects_missing_profiles_and_unknown_adapters() {
    let manifest = LaunchProfileManifest::parse_toml(
        r#"
[profiles.bad]
kind = "server"
source = "server.arcw"
adapter = "custom-http"

[profiles.custom]
kind = "server"
source = "server.arcw"
adapter = "custom-http"
adapter_manifests = ["adapters/custom-http.toml"]
"#,
    )
    .expect("manifest parses");

    assert!(matches!(
        manifest.resolve_profile("missing", Path::new(".")),
        Err(LaunchProfileError::MissingProfile(id)) if id == "missing"
    ));
    assert!(matches!(
        manifest.resolve_profile_with_adapters("bad", Path::new("."), &["native-http"]),
        Err(LaunchProfileError::UnknownAdapter { profile, adapter })
            if profile == "bad" && adapter == "custom-http"
    ));
    let custom = manifest
        .resolve_profile_with_adapters("custom", Path::new("game"), &["native-http"])
        .expect("profile with custom adapter manifest resolves");
    assert_eq!(custom.adapter(), Some("custom-http"));
    assert_eq!(
        custom.adapter_manifests(),
        &[PathBuf::from("game/adapters/custom-http.toml")]
    );
}

#[test]
fn rejects_zero_sized_player_design_viewport() {
    let manifest = LaunchProfileManifest::parse_toml(
        r#"
[profiles.bad]
kind = "game"
source = "game.arcw"

[profiles.bad.player.viewport]
design-width = 0
design-height = 720
fit = "contain"
"#,
    )
    .expect("manifest parses");

    assert!(matches!(
        manifest.resolve_profile("bad", Path::new(".")),
        Err(LaunchProfileError::InvalidPlayerViewport { profile, field })
            if profile == "bad" && field == "design-width"
    ));
}
