fmt:
    @cargo fmt --all

fmt-check:
    @cargo fmt --all --check

clippy:
    @cargo clippy --workspace --all-targets --all-features

structure-audit:
    @cargo +nightly -Zscript tools/structure-audit.rs --root .

structure-audit-gate:
    @cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking

test: test-workspace

test-workspace:
    @cargo test --workspace --lib --tests --exclude arcweft-cli --quiet
    @cargo test -p arcweft-cli --lib --bins --quiet
    @cargo test -p arcweft-cli --test runtime_native_options --quiet
    @cargo test -p arcweft-cli --test check_core_cli --quiet
    @cargo test -p arcweft-cli --test native_style_parity_sample --quiet
    @cargo test -p arcweft-cli --test release_trust_json --quiet
    @cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet
    @cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens --quiet

test-workspace-profile:
    @Write-Host "test-workspace"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); just test-workspace; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }

# Proof persisted-boundary row: execute each real typed owner without a cross-crate codec alias.
awbc_bundle_save_checkpoint_cache_round_trip_without_session_ids:
    @cargo test -p arcweft-core --lib --all-features awbc::tests::canonical_awbc_assertion_payload_round_trips_as_typed_identity -- --exact
    @cargo test -p arcweft-bundle --lib --all-features tests::awfb_round_trip_retains_typed_runtime_assertion_payload -- --exact
    @cargo test -p arcweft-save --test runtime_assertion_identity --all-features save_round_trip_retains_the_real_core_runtime_assertion_payload -- --exact
    @cargo test -p arcweft-project-loader --lib --all-features cache::persistent_query::tests::persistent_query_verified_bytecode_unit_is_actual_hit -- --exact
    @cargo test -p arcweft-core --lib --all-features awbc::tests::fiber_checkpoint_and_serde_preserve_cleanup_stacks -- --exact
    @cargo test -p arcweft-runtime-plan --test api_compile --all-features removed_runtime_plan_apis_are_unavailable -- --exact
    @cargo test -p arcweft-core --test api_compile --all-features runtime_assertion_identity_boundaries_are_compile_time_closed -- --exact

# Proof public-surface row: run the five existing owner trybuild suites directly.
public_api_surface_contains_only_final_nodes:
    @cargo test -p arcweft-lang-syntax --test public_api --all-features removed_public_api_contract -- --exact
    @cargo test -p arcweft-lang-hir --test public_api --all-features removed_and_session_identity_public_api_contract -- --exact
    @cargo test -p arcweft-compiler --test api_compile --all-features removed_compiler_apis_are_unavailable -- --exact
    @cargo test -p arcweft-runtime-plan --test api_compile --all-features removed_runtime_plan_apis_are_unavailable -- --exact
    @cargo test -p arcweft-core --test api_compile --all-features runtime_assertion_identity_boundaries_are_compile_time_closed -- --exact

# Proof recovered-module row: combine executable/cache rejection with tooling-only publication.
recovered_module_is_excluded_from_executable_caches:
    @cargo test -p arcweft-compiler --lib --all-features project::tests::recovered_module_never_enters_runtime_plan_or_compile_cache -- --exact
    @cargo test -p arcweft-lsp --lib --all-features session::publication_gate_tests::recovered_module_is_excluded_from_executable_caches -- --exact

proof-cross-boundary-acceptance: awbc_bundle_save_checkpoint_cache_round_trip_without_session_ids public_api_surface_contains_only_final_nodes recovered_module_is_excluded_from_executable_caches

# Proof E13 Tier 2: execute each ignored production Select boundary independently.
test-proof-select-production-boundaries:
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::expression_lowering::tests::select_limits::e13_tier2_total_slots_exact_uses_three_retired_production_batches -- --ignored --exact
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::expression_lowering::tests::select_limits::e13_tier2_total_slots_one_over_rolls_back_the_direct_select -- --ignored --exact
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::expression_lowering::tests::select_limits::e13_tier2_syntax_expression_limit_uses_a_shallow_select_matrix -- --ignored --exact
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::expression_lowering::tests::select_limits::e13_tier2_hir_expression_limit_uses_a_shallow_select_matrix_and_scope_tail -- --ignored --exact
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::expression_lowering::tests::select_limits::e13_tier2_hir_source_byte_preflight_retains_the_select_pipeline -- --ignored --exact
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::expression_lowering::tests::select_limits::e13_tier2_syntax_diagnostic_limit_uses_real_parser_diagnostics -- --ignored --exact

# Proof Flow T-LIMIT-26 Tier 2: exercise the real attached 65,536/65,537-item boundary.
test-proof-flow-production-boundary:
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::item_lowering::tests::flow_transaction::thread_flow_items_exact_and_one_over_are_atomic_for_attached_flow -- --ignored --exact
    @cargo test -p arcweft-lang-hir --lib --all-features final_lowering::item_lowering::tests::flow_transaction::thread_flow_items_exact_and_one_over_are_atomic_for_attached_thread_expression -- --ignored --exact

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
    @cargo run -p arcweft-cli --quiet -- check --manifest-path samples/rich-text-effects-animation/arcw.toml --profile main

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
    @cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_artifacts_are_well_formed --quiet
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

test-native-aux-capture:
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_read_uri_preserves_animated_image_object_frame_metadata
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_read_uri_preserves_animated_image_layer_frame_pixels
    @cargo test -p arcweft-cli --features native-capture --test check agent_hit_test_capture_time_updates_unpinned_animated_image_frame_metadata
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_captures_clipped_animated_image_object
    @cargo test -p arcweft-cli --features native-capture --test check text_combine_mask_raw_crop
    @cargo test -p arcweft-cli --features native-capture --test check text_combine_object_id_raw_crop
    @cargo test -p arcweft-cli --features native-capture --test check typewriter_text_combine_capture_time_controls_all_glyphs
    @cargo test -p arcweft-cli --features native-capture --test check typewriter_text_combine_capture_time_controls_object_id
    @cargo test -p arcweft-cli --features native-capture --test check typewriter_ruby_capture_time_controls_base_and_annotation
    @cargo test -p arcweft-cli --features native-capture --test check typewriter_ruby_capture_time_controls_object_id

test-tier2: test-slow-mcp test-slow-agent-observe test-native-aux-capture test-visual-golden test-proof-select-production-boundaries test-proof-flow-production-boundary

check-jlreq-punctuation:
    @rustc tools\generate_jlreq_punctuation_data.rs -o target\generate_jlreq_punctuation_data.exe
    @.\target\generate_jlreq_punctuation_data.exe --check

verify: fmt-check check-jlreq-punctuation clippy test-workspace

verify-full: verify test-doc verify-vendor-glyphon test-cli-check-full test-tier2
