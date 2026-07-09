set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

import 'just/bench.just'

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
    @cargo test -p arcweft-core -p arcweft-render-text -p arcweft-text-layout -p arcweft-render-native -p arcweft-player-native --lib --quiet

check-crate crate:
    @cargo check -p {{ crate }}

test-crate crate:
    @cargo test -p {{ crate }} --quiet

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
    @cargo test -p arcweft-render-native image -- --nocapture
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
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_native_renderer_writes_dialogue_layer_framebuffer_crop --quiet -- --exact

test-visual-smoke:
    @cargo test -p arcweft-cli --features native-capture --test check visual_smoke -- --nocapture

test-slow-mcp:
    @cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio -- --ignored --nocapture

test-visual-golden: test-visual-smoke
    @cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
    @cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_renderer_matches_checked_in_imq_golden_fixture -- --ignored --nocapture

native-visual-preflight out:
    @New-Item -ItemType Directory -Force -Path "{{ out }}" | Out-Null
    @cargo +nightly -Zscript tools\write-native-golden-fingerprint.rs --root . --artifact-dir "{{ out }}" --out "{{ out }}\exact-native-golden.environment.json" --status preflight
    @if (-not (Get-Command imq -ErrorAction SilentlyContinue)) { cargo +nightly -Zscript tools\write-native-golden-fingerprint.rs --root . --artifact-dir "{{ out }}" --out "{{ out }}\exact-native-golden.environment.json" --status environment_blocker --blocker missing_imq; Write-Error "native visual artifacts blocked: imq is not available; fingerprint={{ out }}\exact-native-golden.environment.json"; exit 2 }
    @if (-not (Test-Path (Join-Path $env:WINDIR 'Fonts\msmincho.ttc'))) { cargo +nightly -Zscript tools\write-native-golden-fingerprint.rs --root . --artifact-dir "{{ out }}" --out "{{ out }}\exact-native-golden.environment.json" --status environment_blocker --blocker missing_pinned_font; Write-Error "native visual artifacts blocked: MS Mincho font probe failed; fingerprint={{ out }}\exact-native-golden.environment.json"; exit 2 }

native-visual-artifacts out="target\\arcweft-native-capture-artifacts":
    @just native-visual-preflight "{{ out }}"
    @cargo build --release -p arcweft-cli --features native-capture --quiet
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_tutr_golden.arcw --json --image png --out "{{ out }}\vertical_tutr_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_tutr_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_tutr_golden.png "{{ out }}\vertical_tutr_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_tutr_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.arcw --json --image png --out "{{ out }}\vertical_jlreq_preset_loose_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_jlreq_preset_loose_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.png "{{ out }}\vertical_jlreq_preset_loose_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_jlreq_preset_loose_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.arcw --json --image png --out "{{ out }}\vertical_jlreq_preset_normal_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_jlreq_preset_normal_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.png "{{ out }}\vertical_jlreq_preset_normal_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_jlreq_preset_normal_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.arcw --json --image png --out "{{ out }}\vertical_lr_ruby_text_combine_golden.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_lr_ruby_text_combine_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.png "{{ out }}\vertical_lr_ruby_text_combine_golden.candidate.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_lr_ruby_text_combine_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @.\target\release\arcw.exe agent observe tests\fixtures\native_capture\vertical_goal_clear_smoke.arcw --json --image png --out "{{ out }}\vertical_goal_clear_smoke.candidate.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_goal_clear_smoke.observe.json"
    @cargo +nightly -Zscript tools\write-native-golden-fingerprint.rs --root . --artifact-dir "{{ out }}" --out "{{ out }}\exact-native-golden.environment.json" --status artifacts_complete

fixture-refresh-list:
    @Write-Host "Checked-in portable fixtures regenerated by just fixture-refresh:"
    @Write-Host "  web/demo.awfb <- web/arcw.toml profile main"
    @Write-Host "  web/bundle-assets/generated/background.png <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/bundle-assets/generated/character_stand.png <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/bundle-assets/generated/gif_pulse.gif <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "  web/bundle-assets/generated/webp_pulse.webp <- tools/generate-webgpu-demo-assets.rs"
    @Write-Host "Generated deterministic data refreshed by just fixture-refresh:"
    @Write-Host "  crates/arcweft-text-layout/src/jlreq_punctuation_data.rs <- tools/generate_jlreq_punctuation_data.rs"
    @Write-Host "  fixtures/persistent-cache-build/seq04-8-4/goldens/*.json <- just persistent-cache-build-seq04-8-4-goldens-regenerate"
    @Write-Host "Platform-dependent native capture candidates generated by just fixture-refresh-all:"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_tutr_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_jlreq_preset_loose_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_jlreq_preset_normal_golden.png <- matching .arcw source"
    @Write-Host "  target/arcweft-native-capture-refresh/vertical_lr_ruby_text_combine_golden.png <- matching .arcw source"
    @Write-Host "Candidate-only fixture artifacts:"
    @Write-Host "  just native-visual-artifacts [out] writes comparison candidates under target/."
    @Write-Host "  native-visual-artifacts also writes vertical_goal_clear_smoke.candidate.png."
    @Write-Host "  just webgpu-parity writes browser/native comparison artifacts under target/."
    @Write-Host "Authored fixtures not regenerated by this command:"
    @Write-Host "  tests/fixtures/arcw/**/*.arcw"
    @Write-Host "  runtime-driver/runtime-host product AWBC test fixtures constructed in Rust tests"
    @Write-Host "Reference: docs/implementation/fixture-regeneration.md"

fixture-refresh: fixture-refresh-portable fixture-refresh-check

fixture-refresh-portable: fixture-refresh-web-demo-awfb fixture-refresh-webgpu-demo-assets generate-jlreq-punctuation persistent-cache-build-seq04-8-4-goldens-regenerate

fixture-refresh-all: fixture-refresh fixture-refresh-native-capture-candidates fixture-refresh-native-capture-check

fixture-refresh-web-demo-awfb:
    @cargo run -p arcweft-cli --quiet -- bundle --manifest-path web/arcw.toml --profile main --output web/demo.awfb

fixture-refresh-webgpu-demo-assets:
    @cargo +nightly -Zscript tools\generate-webgpu-demo-assets.rs

fixture-refresh-native-capture-candidates out="target\\arcweft-native-capture-refresh":
    @just native-visual-preflight "{{ out }}"
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_tutr_golden.arcw --json --image png --out "{{ out }}\vertical_tutr_golden.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_tutr_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_tutr_golden.png "{{ out }}\vertical_tutr_golden.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_tutr_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.arcw --json --image png --out "{{ out }}\vertical_jlreq_preset_loose_golden.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_jlreq_preset_loose_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_jlreq_preset_loose_golden.png "{{ out }}\vertical_jlreq_preset_loose_golden.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_jlreq_preset_loose_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.arcw --json --image png --out "{{ out }}\vertical_jlreq_preset_normal_golden.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_jlreq_preset_normal_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_jlreq_preset_normal_golden.png "{{ out }}\vertical_jlreq_preset_normal_golden.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_jlreq_preset_normal_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo run -p arcweft-cli --features native-capture --quiet -- agent observe tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.arcw --json --image png --out "{{ out }}\vertical_lr_ruby_text_combine_golden.png" --mode drain --steps 4 --max-ops 64 > "{{ out }}\vertical_lr_ruby_text_combine_golden.observe.json"
    @imq image tests\fixtures\native_capture\vertical_lr_ruby_text_combine_golden.png "{{ out }}\vertical_lr_ruby_text_combine_golden.png" --metrics psnr,ssim,mse,mae,maxae --format json > "{{ out }}\vertical_lr_ruby_text_combine_golden.imq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo +nightly -Zscript tools\write-native-golden-fingerprint.rs --root . --artifact-dir "{{ out }}" --out "{{ out }}\exact-native-golden.environment.json" --status refresh_candidates_complete

fixture-refresh-check:
    @cargo run -p arcweft-cli --quiet -- inspect web/demo.awfb --json | Out-Null
    @cargo test -p arcweft-player-web --test parity --all-features --quiet
    @cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet
    @cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens --quiet

persistent-cache-build-seq04-8-4-goldens:
    @cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens -- --nocapture

persistent-cache-build-seq04-8-4-goldens-regenerate:
    @$env:ARCWEFT_REGENERATE_SEQ04_8_4_GOLDENS = "1"; cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens -- --nocapture

fixture-refresh-native-capture-check:
    @cargo test -p arcweft-cli --features native-capture --test check visual_smoke --quiet -- --nocapture
    @cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet

webgpu-parity:
    @cargo run -p arcweft-cli -- bundle --manifest-path web/arcw.toml --profile main --output web/demo.awfb
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @foreach ($checkpoint in @("focus-first-choice", "hover-second-choice", "press-first-choice", "compact-focus-first-choice", "hidpi-focus-first-choice")) { cargo +nightly -Zscript tools\capture-webgpu-native-frame.rs --output "target\webgpu-parity\native-$checkpoint.png" --checkpoint $checkpoint --visual-time-millis 160 --target-format rgba8unorm; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @$checkpoints = @("focus-first-choice", "hover-second-choice", "press-first-choice", "compact-focus-first-choice", "hidpi-focus-first-choice"); $env:ARW_WEB_PARITY_DIR = (Resolve-Path target\webgpu-parity).Path; $env:ARW_WEB_PARITY_CHECKPOINTS = ($checkpoints -join ","); npm.cmd --prefix web test; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @foreach ($checkpoint in @("focus-first-choice", "hover-second-choice")) { cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native "target\webgpu-parity\native-$checkpoint.png" --web "target\webgpu-parity\web-$checkpoint.png" --report "target\webgpu-parity\parity-$checkpoint.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native target\webgpu-parity\native-press-first-choice.png --web target\webgpu-parity\web-press-first-choice.png --report target\webgpu-parity\parity-press-first-choice.json --min-psnr 23.9
    @cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native target\webgpu-parity\native-compact-focus-first-choice.png --web target\webgpu-parity\web-compact-focus-first-choice.png --report target\webgpu-parity\parity-compact-focus-first-choice.json --min-psnr 21.4 --max-mse 0.0072 --max-mae 0.0108 --max-changed-pixel-ratio 0.03
    @cargo +nightly -Zscript tools\verify-webgpu-parity.rs --native target\webgpu-parity\native-hidpi-focus-first-choice.png --web target\webgpu-parity\web-hidpi-focus-first-choice.png --report target\webgpu-parity\parity-hidpi-focus-first-choice.json --min-psnr 20.0 --max-mse 0.0101 --max-mae 0.0168 --max-changed-pixel-ratio 0.04
    @foreach ($checkpoint in @("focus-first-choice", "hover-second-choice", "press-first-choice", "compact-focus-first-choice", "hidpi-focus-first-choice")) { imq compare "target\webgpu-parity\native-$checkpoint.png" "target\webgpu-parity\web-$checkpoint.png" --format json --output "target\webgpu-parity\imq-$checkpoint.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }

css-style-parity:
    @New-Item -ItemType Directory -Force -Path web\local,target\css-style-parity | Out-Null
    @cargo run -p arcweft-cli -- bundle samples/css-style-parity/main.arcw --output web/local/css-style-parity.awfb
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @cargo +nightly -Zscript tools\capture-css-style-parity-native-frame.rs --bundle web/local/css-style-parity.awfb --font web\assets\arcweft-demo.ttf --output target\css-style-parity\native-default.png --frame-report target\css-style-parity\native-default.frame.json --viewport default --visual-time-millis 9000 --target-format rgba8unorm
    @cargo +nightly -Zscript tools\capture-css-style-parity-native-frame.rs --bundle web/local/css-style-parity.awfb --font web\assets\arcweft-demo.ttf --output target\css-style-parity\native-compact.png --frame-report target\css-style-parity\native-compact.frame.json --viewport compact --visual-time-millis 9000 --target-format rgba8unorm
    @cargo +nightly -Zscript tools\capture-css-style-parity-native-frame.rs --bundle web/local/css-style-parity.awfb --font web\assets\arcweft-demo.ttf --output target\css-style-parity\native-hidpi.png --frame-report target\css-style-parity\native-hidpi.frame.json --viewport hidpi --visual-time-millis 9000 --target-format rgba8unorm
    @$env:ARW_CSS_STYLE_PARITY_DIR = (Resolve-Path target\css-style-parity).Path; $env:ARW_CSS_STYLE_PARITY_CHECKPOINTS = "default,compact,hidpi"; $env:ARW_CSS_STYLE_PARITY_VISUAL_TIME_MILLIS = "9000"; $env:ARW_CSS_STYLE_PARITY_FONT_URL = "./assets/arcweft-demo.ttf"; node web\tests\css-style-parity-smoke.mjs; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo +nightly -Zscript tools\run-css-style-parity-gates.rs --dir target\css-style-parity --font web\assets\arcweft-demo.ttf

css-style-parity-profile:
    @cargo +nightly -Zscript tools\profile-css-style-parity-startup.rs --output target\css-style-parity\startup-profile.json

css-layout-cascade-coverage:
    @cargo test -p arcweft-takumi-adapter css_layout_cascade --quiet

reactive-view-style-sample:
    @New-Item -ItemType Directory -Force -Path web\local,target\reactive-view-style,target\reactive-view-style\interaction-states | Out-Null
    @cargo run -p arcweft-cli -- bundle --manifest-path samples/reactive-view-style/arcw.toml --profile main --output web/local/reactive-view-style.awfb
    @cargo run -p arcweft-render-wgpu --example view_interaction_showcase -- --out target\reactive-view-style\interaction-states

web-player-refresh:
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown --all-features
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @cargo run -p arcweft-cli --all-features -- bundle --manifest-path web\arcw.toml --profile main --output web\demo.awfb
    @cargo run -p arcweft-cli --all-features -- bundle --manifest-path samples\modern-feedback-view\arcw.toml --profile main --output web\modern-feedback-view.awfb

web-player-serve port="4173":
    @Write-Host "Serving Arcweft web player at http://127.0.0.1:{{ port }}/"
    @Write-Host "Modern feedback: http://127.0.0.1:{{ port }}/?bundle=./modern-feedback-view.awfb"
    @python -m http.server {{ port }} --bind 127.0.0.1 --directory web

ime-sample-web port="8786":
    @cargo +nightly -Zscript tools\build-web-ime-player-rendered-fixture.rs --out web\ime-player-rendered.awfb
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @Write-Host "Serving Arcweft player-rendered IME sample at http://127.0.0.1:{{ port }}/ime-sample.html"
    @Write-Host "Equivalent player URL: http://127.0.0.1:{{ port }}/index.html?bundle=./ime-player-rendered.awfb"
    @python -m http.server {{ port }} --bind 127.0.0.1 --directory web

ime-sample-native:
    @cargo run -p arcweft-cli --features native-player -- run --runner native samples/native-text-input/src/main.arcw --text-input-trace-out target\native-text-input-trace\native-player-ime.real.json

view-text-input-native-smoke-check:
    @cargo run -p arcweft-cli -- check --manifest-path samples\native-text-input\arcw.toml
    @cargo run -p arcweft-cli -- check --manifest-path samples\text-submit-flow\arcw.toml
    @cargo run -p arcweft-cli -- check --manifest-path samples\modern-feedback-view\arcw.toml
    @cargo run -p arcweft-cli -- bundle samples\native-text-input\src\main.arcw --output target\arcweft\native-text-input-seq06.16.3.awfb
    @cargo run -p arcweft-cli -- bundle samples\text-submit-flow\src\main.arcw --output target\arcweft\text-submit-flow-seq06.16.3.awfb
    @cargo run -p arcweft-cli -- bundle --manifest-path samples\modern-feedback-view\arcw.toml --profile main --output target\arcweft\modern-feedback-view-seq06.16.3.awfb

view-text-input-native-smoke out="target\\native-text-input-trace\\seq06.16.3":
    @just view-text-input-native-smoke-check
    @New-Item -ItemType Directory -Force -Path "{{ out }}" | Out-Null
    @cargo run -p arcweft-cli --features native-player -- run --runner native samples\native-text-input\src\main.arcw --text-input-trace-out "{{ out }}\native-player-ime.real.json"
    @cargo +nightly -Zscript tools\verify-seq06-16-3-native-smoke-trace.rs --trace "{{ out }}\native-player-ime.real.json"

ime-sample-native-contract:
    @cargo run -p arcweft-desktop-native --example ime_text_input_contract

ime-sample-check:
    @cargo +nightly -Zscript tools\build-web-ime-player-rendered-fixture.rs --out web\ime-player-rendered.awfb
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @npm.cmd --prefix web run test:ime
    @cargo test -p arcweft-render-wgpu focused_text_input_target --all-features --quiet
    @cargo test -p arcweft-player-scene --test runtime_text_controls --quiet
    @cargo test -p arcweft-player-web runtime_text_input --all-features --quiet

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

seq06-13e1-inset-shadow-native-capture out="target\\seq06.13e.1-inset-box-shadow-golden":
    @cargo +nightly -Zscript tools\capture-seq06-13e1-inset-shadow-native-frame.rs --root . --out-dir "{{ out }}"

seq06-13e1-inset-shadow-pinned-native-golden out="target\\seq06.13e.1-inset-box-shadow-golden":
    @cargo +nightly -Zscript tools\collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir "{{ out }}" --mode native --run

seq06-13e1-inset-shadow-pinned-golden out="target\\seq06.13e.1-inset-box-shadow-golden":
    @cargo +nightly -Zscript tools\collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir "{{ out }}" --mode both --run
    @cargo test -p arcweft-render-wgpu --test view_box_shadow_exact_png_golden --all-features -- --ignored --nocapture

generate-jlreq-punctuation:
    @rustc tools\generate_jlreq_punctuation_data.rs -o target\generate_jlreq_punctuation_data.exe
    @.\target\generate_jlreq_punctuation_data.exe --apply

check-jlreq-punctuation:
    @rustc tools\generate_jlreq_punctuation_data.rs -o target\generate_jlreq_punctuation_data.exe
    @.\target\generate_jlreq_punctuation_data.exe --check

verify: fmt-check check-jlreq-punctuation clippy test-workspace

verify-full: verify test-doc verify-vendor-glyphon test-cli-check-full test-tier2
