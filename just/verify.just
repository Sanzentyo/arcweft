fmt:
    @cargo fmt --all

fmt-check:
    @cargo fmt --all --check

clippy:
    @cargo clippy --workspace --all-targets --all-features

test: test-workspace

test-workspace:
    @cargo test --workspace --lib --tests --exclude arcweft-cli --quiet
    @cargo test -p arcweft-cli --lib --bins --quiet
    @cargo test -p arcweft-cli --test runtime_native_options --quiet
    @cargo test -p arcweft-cli --test check_core_cli --quiet
    @cargo test -p arcweft-cli --test css_style_parity_sample --quiet
    @cargo test -p arcweft-cli --test release_trust_json --quiet
    @cargo test -p arcweft-cli --test responsive_stage_placement --quiet
    @cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet
    @cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens --quiet

test-workspace-profile:
    @Write-Host "test-workspace"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); just test-workspace; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }

test-doc:
    @cargo test --workspace --doc --quiet

test-fast:
    @cargo test -p arcweft-core -p arcweft-render-text -p arcweft-text-layout -p arcweft-render-wgpu -p arcweft-player-native --lib --quiet

check-crate crate:
    @cargo check -p {{ crate }}

test-crate crate:
    @cargo test -p {{ crate }} --quiet

test-rich-text:
    @cargo test -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-runtime-plan --lib --tests --quiet
    @cargo test -p arcweft-render-text -p arcweft-text-layout -p arcweft-render-wgpu -p arcweft-player-native --lib --quiet
    @cargo run -p arcweft-cli --quiet -- check samples/rich-text-fx.arcw
    @just test-cli-native

test-rich-text-object-goal:
    @cargo test -p arcweft-agent-protocol -- --nocapture
    @cargo test -p arcweft-agent-mcp -- --nocapture
    @cargo test -p arcweft-render-wgpu fx -- --nocapture
    @cargo test -p arcweft-render-wgpu prepared -- --nocapture
    @cargo test -p arcweft-render-wgpu offscreen -- --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_reports_text_presentation_z_index_depth -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_hit_test_capture_time_follows_animated_text_proxy_bounds -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_native_renderer_captures_combined_typewriter_animation_sample -- --ignored --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_native_rich_text_reports_missing_motion_diagnostics_in_image_resources -- --exact --nocapture
    @cargo run -p arcweft-cli --quiet -- check samples/rich-text-full-grammar.arcw
    @cargo run -p arcweft-cli --quiet -- check samples/rich-text-effects-animation.arcw

test-image-animation-goal:
    @cargo test -p arcweft-image -- --nocapture
    @cargo test -p arcweft-presentation image -- --nocapture
    @cargo test -p arcweft-view image -- --nocapture
    @cargo test -p arcweft-render-wgpu image -- --nocapture
    @cargo test -p arcweft-lang-sema tests::declarations::parses_surface_alias_and_resource_entity_families -- --exact --nocapture
    @cargo test -p arcweft-lang-sema tests::typecheck::typechecks_presentation_image_object_call_with_named_asset_and_bounds -- --exact --nocapture
    @cargo test -p arcweft-cli app::image_declarations -- --nocapture
    @cargo test -p arcweft-cli app::bundle::tests::static_image_asset_refs_collects_declared_image_object_assets -- --exact --nocapture
    @cargo test -p arcweft-cli --test check bundle_json_packages_image_animation_sample_assets_and_run_bundle_validates_them -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_read_uri_preserves_animated_image_object_frame_metadata -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_read_uri_preserves_animated_image_layer_frame_pixels -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_mcp_tool_result_preserves_animated_image_object_metadata_and_raw_blob -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_hit_test_reports_animated_image_object_proxy_metadata -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_hit_test_capture_time_updates_unpinned_animated_image_frame_metadata -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_image_alignment_sample_uses_authored_alignment_geometry -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_native_captures_clipped_animated_image_object -- --exact --nocapture
    @cargo run -p arcweft-cli --quiet -- check samples/image-animation.arcw

test-cli-check:
    @cargo test -p arcweft-cli --test check bench_json --quiet
    @cargo test -p arcweft-cli --test check run_json --quiet
    @cargo test -p arcweft-cli --test check jit_check_json --quiet
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_json --quiet
    @just test-cli-native

test-cli-check-full:
    @cargo test -p arcweft-cli --features native-capture --test check --quiet

test-cli-native: test-visual-smoke
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_shared_renderer_writes_dialogue_layer_masked_framebuffer_crop --quiet -- --exact

test-visual-smoke:
    @cargo test -p arcweft-cli --features native-capture --test check visual_smoke -- --nocapture

test-slow-mcp:
    @cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio -- --ignored --nocapture

test-visual-golden: test-visual-smoke
    @cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_renderer_matches_checked_in_imq_golden_fixture -- --ignored --nocapture

check-vendor-glyphon:
    @cargo check --manifest-path vendor\glyphon\Cargo.toml

clippy-vendor-glyphon:
    @cargo clippy --manifest-path vendor\glyphon\Cargo.toml --lib --tests -- -D warnings -A clippy::too_many_arguments

test-vendor-glyphon:
    @cargo test --manifest-path vendor\glyphon\Cargo.toml --lib

verify-vendor-glyphon: check-vendor-glyphon clippy-vendor-glyphon test-vendor-glyphon

test-slow-agent-observe:
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_writes_layer_png_and_object_raw_images -- --ignored --nocapture

test-tier2: test-slow-mcp test-slow-agent-observe test-visual-golden

check-jlreq-punctuation:
    @rustc tools\generate_jlreq_punctuation_data.rs -o target\generate_jlreq_punctuation_data.exe
    @.\target\generate_jlreq_punctuation_data.exe --check

verify: fmt-check check-jlreq-punctuation clippy test-workspace

verify-full: verify test-doc verify-vendor-glyphon test-cli-check-full test-tier2
