set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

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
    @cargo test -p arcweft-cli --test regression_harness --quiet
    @cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet

test-workspace-profile:
    @Write-Host "workspace-no-run-excluding-cli"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); cargo test --workspace --lib --tests --exclude arcweft-cli --no-run --quiet; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }
    @Write-Host "workspace-list-excluding-cli"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); cargo test --workspace --lib --tests --exclude arcweft-cli --quiet -- --list | Out-Null; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }
    @Write-Host "workspace-lib-tests-excluding-cli"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); cargo test --workspace --lib --tests --exclude arcweft-cli --quiet; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }
    @Write-Host "cli-lib-bins"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); cargo test -p arcweft-cli --lib --bins --quiet; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }
    @Write-Host "cli-regression-harness"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); cargo test -p arcweft-cli --test regression_harness --quiet; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }
    @Write-Host "cli-fixtures-check-run"; $sw = [System.Diagnostics.Stopwatch]::StartNew(); cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet; $code = $LASTEXITCODE; $sw.Stop(); Write-Host ("elapsed_seconds={0:N3}" -f $sw.Elapsed.TotalSeconds); if ($code -ne 0) { exit $code }

test-doc:
    @cargo test --workspace --doc --quiet

test-fast:
    @cargo test -p arcweft-core -p arcweft-render-text -p arcweft-text-layout -p arcweft-render-native -p arcweft-player-native --lib --quiet

check-crate crate:
    @cargo check -p {{crate}}

test-crate crate:
    @cargo test -p {{crate}} --quiet

test-rich-text:
    @cargo test -p arcweft-render-text -p arcweft-text-layout -p arcweft-render-native -p arcweft-player-native --lib --quiet
    @just test-cli-native

test-rich-text-object-goal:
    @cargo test -p arcweft-agent-protocol -- --nocapture
    @cargo test -p arcweft-agent-mcp -- --nocapture
    @cargo test -p arcweft-render-native motion -- --nocapture
    @cargo test -p arcweft-render-native shader -- --nocapture
    @cargo test -p arcweft-render-native post_process -- --nocapture
    @cargo test -p arcweft-render-native typewriter -- --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_reports_text_presentation_z_index_depth -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_hit_test_capture_time_follows_animated_text_proxy_bounds -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_renderer_captures_combined_typewriter_animation_sample -- --ignored --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_rich_text_reports_missing_motion_diagnostics_in_image_resources -- --exact --nocapture
    @cargo run -p arcweft-cli --quiet -- check samples/rich-text-full-grammar.arcw
    @cargo run -p arcweft-cli --quiet -- check samples/rich-text-effects-animation.arcw

test-image-animation-goal:
    @cargo test -p arcweft-image -- --nocapture
    @cargo test -p arcweft-presentation image -- --nocapture
    @cargo test -p arcweft-ui image -- --nocapture
    @cargo test -p arcweft-render-native image -- --nocapture
    @cargo test -p arcweft-lang-sema tests::declarations::parses_surface_alias_and_resource_entity_families -- --exact --nocapture
    @cargo test -p arcweft-lang-sema tests::typecheck::typechecks_presentation_image_object_call_with_named_asset_and_bounds -- --exact --nocapture
    @cargo test -p arcweft-cli app::image_declarations -- --nocapture
    @cargo test -p arcweft-cli app::bundle::tests::static_image_asset_refs_collects_declared_image_object_assets -- --exact --nocapture
    @cargo test -p arcweft-cli --test check bundle_json_packages_image_animation_sample_assets_and_run_bundle_validates_them -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_read_uri_preserves_animated_image_object_frame_metadata -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_read_uri_preserves_animated_image_layer_frame_pixels -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_mcp_tool_result_preserves_animated_image_object_metadata_and_raw_blob -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_hit_test_reports_animated_image_object_proxy_metadata -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_hit_test_capture_time_updates_unpinned_animated_image_frame_metadata -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_image_alignment_sample_uses_authored_alignment_geometry -- --exact --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_captures_clipped_animated_image_object -- --exact --nocapture
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
    @foreach ($test in @("agent_observe_native_renderer_writes_framebuffer_png", "agent_observe_native_renderer_writes_dialogue_layer_framebuffer_crop", "agent_observe_native_renderer_writes_object_raw_crop", "agent_observe_native_renderer_writes_textbox_mask_as_glyph_geometry", "agent_observe_native_renderer_writes_textbox_object_id_as_glyph_geometry")) { cargo test -p arcweft-cli --features native-capture --test check $test --quiet -- --exact; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }

test-visual-smoke:
    @cargo test -p arcweft-cli --features native-capture --test check visual_smoke -- --nocapture

test-profile:
    @Write-Host "workspace-no-run"; Measure-Command { cargo test --workspace --no-run --quiet }
    @Write-Host "workspace-lib-tests"; Measure-Command { cargo test --workspace --lib --tests --quiet }
    @Write-Host "workspace-doc"; Measure-Command { cargo test --workspace --doc --quiet }
    @Write-Host "workspace-all"; Measure-Command { cargo test --workspace --quiet }
    @Write-Host "test-fast"; Measure-Command { cargo test -p arcweft-core -p arcweft-render-text -p arcweft-text-layout -p arcweft-render-native -p arcweft-player-native --lib --quiet }
    @Write-Host "cli-check"; Measure-Command { cargo test -p arcweft-cli --features native-capture --test check --quiet }
    @Write-Host "cli-native"; Measure-Command { just test-cli-native }
    @Write-Host "bench-json"; Measure-Command { cargo test -p arcweft-cli --test check bench_json --quiet }
    @Write-Host "run-json"; Measure-Command { cargo test -p arcweft-cli --test check run_json --quiet }
    @Write-Host "jit-check-json"; Measure-Command { cargo test -p arcweft-cli --test check jit_check_json --quiet }

test-slow-mcp:
    @cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio -- --ignored --nocapture

test-visual-golden: test-visual-smoke
    @cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_renderer_matches_checked_in_imq_golden_fixtures -- --ignored --nocapture

native-visual-artifacts out="target\\arcweft-native-capture-artifacts":
    @New-Item -ItemType Directory -Force -Path "{{out}}" | Out-Null
    @cargo build --release -p arcweft-cli --features native-capture --quiet
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_tutr_golden.arcw --json --image png --out "{{out}}\vertical_tutr_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_tutr_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_tutr_golden.png "{{out}}\vertical_tutr_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{out}}\vertical_tutr_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.arcw --json --image png --out "{{out}}\vertical_jlreq_preset_loose_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_jlreq_preset_loose_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.png "{{out}}\vertical_jlreq_preset_loose_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{out}}\vertical_jlreq_preset_loose_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.arcw --json --image png --out "{{out}}\vertical_jlreq_preset_normal_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_jlreq_preset_normal_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.png "{{out}}\vertical_jlreq_preset_normal_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{out}}\vertical_jlreq_preset_normal_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.arcw --json --image png --out "{{out}}\vertical_lr_ruby_text_combine_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_lr_ruby_text_combine_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.png "{{out}}\vertical_lr_ruby_text_combine_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{out}}\vertical_lr_ruby_text_combine_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_goal_clear_smoke.arcw --json --image png --out "{{out}}\vertical_goal_clear_smoke.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_goal_clear_smoke.observe.json"

fixture-refresh-list:
    @Write-Host "Checked-in portable fixtures regenerated by just fixture-refresh:"
    @Write-Host "  web/demo.awfb <- web/demo.arcw"
    @Write-Host "  web/assets/generated-background.png <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/assets/generated-character.png <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/assets/generated-pulse.gif <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/assets/generated-pulse.webp <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/.arcweft/asset/generated/* <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "Generated deterministic data refreshed by just fixture-refresh:"
    @Write-Host "  crates/arcweft-lang-syntax/src/jlreq_punctuation_data.rs <- tools/generate_jlreq_punctuation_data.rs"
    @Write-Host "Platform-dependent native capture candidates generated by just fixture-refresh-all:"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_tutr_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_jlreq_preset_loose_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_jlreq_preset_normal_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_lr_ruby_text_combine_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-artifacts/vertical_goal_clear_smoke.candidate.png <- smoke-only visual artifact"
    @Write-Host "Candidate-only fixture artifacts:"
    @Write-Host "  just native-visual-artifacts [out] writes comparison candidates under target/."
    @Write-Host "  just webgpu-parity writes browser/native comparison artifacts under target/."
    @Write-Host "Authored fixtures not regenerated by this command:"
    @Write-Host "  tests/fixtures/arcw/**/*.arcw"
    @Write-Host "  runtime-driver/runtime-host product AWBC test fixtures constructed in Rust tests"
    @Write-Host "Reference: docs/implementation/fixture-regeneration.md"

fixture-refresh: fixture-refresh-portable fixture-refresh-check

fixture-refresh-portable: fixture-refresh-web-demo-awfb fixture-refresh-webgpu-demo-assets generate-jlreq-punctuation

fixture-refresh-all: fixture-refresh fixture-refresh-native-capture-candidates fixture-refresh-native-capture-check

fixture-refresh-web-demo-awfb:
    @cargo run -p arcweft-cli --quiet -- bundle web/demo.arcw --output web/demo.awfb

fixture-refresh-webgpu-demo-assets:
    @cargo +nightly -Zscript tools\generate-webgpu-demo-assets.rs

fixture-refresh-native-capture-candidates out="target\\arcweft-native-capture-refresh":
    @New-Item -ItemType Directory -Force -Path "{{out}}" | Out-Null
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_tutr_golden.arcw --json --image png --out "{{out}}\vertical_tutr_golden.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_tutr_golden.observe.json"
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.arcw --json --image png --out "{{out}}\vertical_jlreq_preset_loose_golden.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_jlreq_preset_loose_golden.observe.json"
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.arcw --json --image png --out "{{out}}\vertical_jlreq_preset_normal_golden.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_jlreq_preset_normal_golden.observe.json"
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.arcw --json --image png --out "{{out}}\vertical_lr_ruby_text_combine_golden.png" --mode drain --steps 4 --max-ops 64 > "{{out}}\vertical_lr_ruby_text_combine_golden.observe.json"

fixture-refresh-check:
    @cargo run -p arcweft-cli --quiet -- inspect web/demo.awfb --json | Out-Null
    @cargo test -p arcweft-player-web --test parity --all-features --quiet
    @cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet

fixture-refresh-native-capture-check:
    @cargo test -p arcweft-cli --features native-capture --test check visual_smoke --quiet -- --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet

webgpu-parity:
    @cargo run -p arcweft-cli -- bundle web/demo.arcw --output web/demo.awfb
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @foreach ($checkpoint in @("focus-first-choice", "hover-second-choice", "press-first-choice", "compact-focus-first-choice", "hidpi-focus-first-choice")) { cargo +nightly -Zscript tools\capture-webgpu-native-frame.rs --output "target\webgpu-parity\native-$checkpoint.png" --checkpoint $checkpoint --visual-time-millis 160 --target-format rgba8unorm; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @$checkpoints = @("focus-first-choice", "hover-second-choice", "press-first-choice", "compact-focus-first-choice", "hidpi-focus-first-choice"); $env:ARW_WEB_PARITY_DIR = (Resolve-Path target\webgpu-parity).Path; $env:ARW_WEB_PARITY_CHECKPOINTS = ($checkpoints -join ","); npm.cmd --prefix web test; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @foreach ($checkpoint in @("focus-first-choice", "hover-second-choice")) { cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native "target\webgpu-parity\native-$checkpoint.png" --web "target\webgpu-parity\web-$checkpoint.png" --report "target\webgpu-parity\parity-$checkpoint.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native target\webgpu-parity\native-press-first-choice.png --web target\webgpu-parity\web-press-first-choice.png --report target\webgpu-parity\parity-press-first-choice.json --min-psnr 23.9
    @cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native target\webgpu-parity\native-compact-focus-first-choice.png --web target\webgpu-parity\web-compact-focus-first-choice.png --report target\webgpu-parity\parity-compact-focus-first-choice.json --min-psnr 21.4 --max-mse 0.0072 --max-mae 0.0108 --max-changed-pixel-ratio 0.03
    @cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native target\webgpu-parity\native-hidpi-focus-first-choice.png --web target\webgpu-parity\web-hidpi-focus-first-choice.png --report target\webgpu-parity\parity-hidpi-focus-first-choice.json --min-psnr 20.0 --max-mse 0.0101 --max-mae 0.0168 --max-changed-pixel-ratio 0.04
    @foreach ($checkpoint in @("focus-first-choice", "hover-second-choice", "press-first-choice", "compact-focus-first-choice", "hidpi-focus-first-choice")) { imq compare "target\webgpu-parity\native-$checkpoint.png" "target\webgpu-parity\web-$checkpoint.png" --format json --output "target\webgpu-parity\imq-$checkpoint.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }

ime-sample-web port="8786":
    @Write-Host "Serving Arcweft IME sample at http://127.0.0.1:{{port}}/ime-sample.html"
    @python -m http.server {{port}} --bind 127.0.0.1 --directory web

ime-sample-native:
    @cargo run -p arcweft-desktop-native --example ime_text_input_contract

ime-sample-check:
    @node web\tests\ime-sample-source.mjs
    @node web\tests\ime-sample-smoke.mjs
    @cargo run -p arcweft-desktop-native --example ime_text_input_contract

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

generate-jlreq-punctuation:
    @rustc tools\generate_jlreq_punctuation_data.rs -o target\generate_jlreq_punctuation_data.exe
    @.\target\generate_jlreq_punctuation_data.exe --apply

check-jlreq-punctuation:
    @rustc tools\generate_jlreq_punctuation_data.rs -o target\generate_jlreq_punctuation_data.exe
    @.\target\generate_jlreq_punctuation_data.exe --check

regression:
    @cargo test -p arcweft-cli --test regression_harness

scan-absolute-paths:
    @cargo test -p arcweft-cli checked_in_docs_and_samples_do_not_record_host_absolute_paths --test regression_harness

scan-removed-dsl:
    @cargo test -p arcweft-cli source_tree_does_not_reintroduce_removed_whitespace_command_dsl_or_shims --test regression_harness

verify: fmt-check check-jlreq-punctuation clippy test-workspace scan-absolute-paths scan-removed-dsl

verify-full: verify test-doc verify-vendor-glyphon test-tier2

toolchain-profile-pure-jit-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-003 --command bench-009 --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-aot-object-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-009-aot-object --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-width-fast-path-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-033-width-jit --command bench-033-width-aot --command bench-033-width-vm --command bench-040-width-jit --command bench-040-width-aot --command bench-040-width-vm --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-width-release-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-033-width-jit-release --command bench-033-width-aot-release --command bench-033-width-vm-release --command bench-040-width-jit-release --command bench-040-width-aot-release --command bench-040-width-vm-release --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-width-object-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-033-width-aot-object --command bench-040-width-aot-object --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-bias --command math-matrix-add --command math-tensor-add --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-f64-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-f64 --command math-matrix-add-f64 --command math-tensor-add-f64 --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-wgpu-reuse-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-bias-wgpu-reuse --command math-matrix-add-wgpu-reuse --command math-tensor-add-wgpu-reuse --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-auto-wgpu-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-auto-wgpu --command math-matmul-bias-auto-wgpu-reuse --command math-matrix-add-auto-wgpu-reuse --command math-tensor-add-auto-wgpu-reuse --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-flow-math-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command flow-math-matmul-glam --command flow-math-matrix-add-ndarray --command flow-math-tensor-add-ndarray --command flow-math-matmul-f64-ndarray --command flow-math-matrix-add-f64-ndarray --command flow-math-tensor-add-f64-ndarray --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-flow-math-auto-wgpu-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command flow-math-matmul-auto-wgpu --repeat {{repeat}} --warmup {{warmup}} --json

bench-009:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit --pure-workers 4 --pure-batch-min-len 64

bench-002:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-003:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-005:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-007:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 128 --max-ops 128 --pure-backend jit

bench-008:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/008_large_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend auto

bench-010:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/010_dense_i32_sum.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-011:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/011_dense_u64_sum.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-012:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-013:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-014:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-015:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-016:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-017:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-018:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-019:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-020:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-022:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-023:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-029:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-030:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-031:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-032:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-033:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit

bench-036:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-037:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-038:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/038_wide_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit

bench-039:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/039_hot_for_pure_auto_jit.arcw --json --iterations 4 --warmup 1 --samples 3 --steps 512 --max-ops 512 --pure-backend auto

bench-040:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit

bench-numeric-vm:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/010_dense_i32_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/011_dense_u64_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64

bench-numeric-pure-vm:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm

bench-numeric-aot:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot

bench-numeric-jit:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 128 --max-ops 128 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/008_large_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend auto
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend auto
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/038_wide_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/039_hot_for_pure_auto_jit.arcw --json --iterations 4 --warmup 1 --samples 3 --steps 512 --max-ops 512 --pure-backend auto
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit

bench-024:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend glam --value lhs=matrix/f32/4x4:1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 --value rhs=matrix/f32/4x4:2,0,0,0,0,2,0,0,0,0,2,0,0,0,0,2

bench-024-wgpu-auto:
    @cargo run -p arcweft-cli --features math-wgpu --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw --json --iterations 5 --warmup 2 --samples 5 --steps 64 --max-ops 64 --math-backend auto --math-wgpu-min-elements 1 --value lhs=matrix/f32/8x8:1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1 --value rhs=matrix/f32/8x8:2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2

bench-025:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/025_matrix_add_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f32/4x4:1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16 --value rhs=matrix/f32/4x4:16,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1

bench-026:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/026_tensor_add_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=tensor/f32/2x2x2:1,2,3,4,5,6,7,8 --value rhs=tensor/f32/2x2x2:8,7,6,5,4,3,2,1

bench-027:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/027_matrix_matmul_f64.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2,3.25,4.5 --value rhs=matrix/f64/2x2:5,6.5,7,8.25

bench-028:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/028_tensor_add_f64.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=tensor/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=tensor/f64/2x2:5,6.25,7.5,8.75

bench-035:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/035_matrix_add_f64.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=matrix/f64/2x2:5,6.25,7.5,8.75

bench-thread:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw --json --iterations 10 --warmup 2 --samples 5 --steps 64 --max-ops 64

bench-system:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw --json --iterations 1 --warmup 0 --samples 3 --steps 24 --max-ops 24 --mode drain

bench-math-cpu:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul --size 64 --iterations 10 --warmup 2

bench-math-glam:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul --size 4 --iterations 50 --warmup 5

bench-math-wgpu:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matmul --size 512 --iterations 3 --warmup 1

bench-math-matmul-bias:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul-bias-add --size 64 --iterations 10 --warmup 2

bench-math-f64:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul-f64 --size 64 --iterations 10 --warmup 2
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matrix-add-f64 --size 1024 --iterations 5 --warmup 1
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op tensor-add-f64 --size 1024 --iterations 5 --warmup 1

bench-math-matmul-bias-wgpu:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matmul-bias-add --size 512 --iterations 3 --warmup 1

bench-math-matmul-bias-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse

bench-math-matmul-bias-submit-only:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 512 --iterations 3 --warmup 1 --submit-only

bench-math-matmul-bias-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matmul-bias-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse-capacity

bench-math-inference-matmul-bias-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op inference-matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse

bench-math-inference-matmul-bias-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op inference-matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matmul-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul --size 128 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matmul-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul --size 128 --iterations 5 --warmup 1 --reuse-capacity

bench-math-matrix-add:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matrix-add --size 4096 --iterations 5 --warmup 1

bench-math-tensor-add:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op tensor-add --size 4096 --iterations 5 --warmup 1

bench-math-matrix-add-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 4096 --iterations 5 --warmup 1 --reuse

bench-math-matrix-add-submit-only:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 4096 --iterations 3 --warmup 1 --submit-only

bench-math-matrix-add-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 64 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matrix-add-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 64 --iterations 5 --warmup 1 --reuse-capacity

bench-math-tensor-add-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 4096 --iterations 5 --warmup 1 --reuse

bench-math-tensor-add-submit-only:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 4096 --iterations 3 --warmup 1 --submit-only

bench-math-tensor-add-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 64 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-tensor-add-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 64 --iterations 5 --warmup 1 --reuse-capacity

browser-webgpu-bench-check:
    @cargo check -p arcweft-browser-bench --target wasm32-unknown-unknown --all-features
    @node --test crates/arcweft-browser-bench/web/chrome-smoke-summary.test.mjs

browser-webgpu-bench-build:
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build

browser-webgpu-bench-serve port="8787":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build-and-serve --port {{port}}

browser-webgpu-bench-smoke port="8787":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}}

browser-webgpu-bench-perf port="8788":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset perf --timeout-ms 180000

browser-webgpu-bench-isolate port="8789":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset isolate --timeout-ms 180000

browser-webgpu-bench-stability port="8790":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset stability --timeout-ms 240000

browser-webgpu-bench-capacity-stability port="8791":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset capacity-stability --timeout-ms 300000

browser-webgpu-bench-submit-only port="8792":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset submit-only --timeout-ms 300000
