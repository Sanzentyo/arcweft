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
    @cargo +nightly -Zscript tools\capture-text-parity-frame.rs --bundle web/local/css-style-parity.awfb --checkpoint default --font web\assets\arcweft-demo.ttf --output target\css-style-parity\native-default.png --frame-report target\css-style-parity\native-default.frame.json --viewport default --visual-time-millis 9000 --target-format rgba8unorm
    @cargo +nightly -Zscript tools\capture-text-parity-frame.rs --bundle web/local/css-style-parity.awfb --checkpoint compact --font web\assets\arcweft-demo.ttf --output target\css-style-parity\native-compact.png --frame-report target\css-style-parity\native-compact.frame.json --viewport compact --visual-time-millis 9000 --target-format rgba8unorm
    @cargo +nightly -Zscript tools\capture-text-parity-frame.rs --bundle web/local/css-style-parity.awfb --checkpoint hidpi --font web\assets\arcweft-demo.ttf --output target\css-style-parity\native-hidpi.png --frame-report target\css-style-parity\native-hidpi.frame.json --viewport hidpi --visual-time-millis 9000 --target-format rgba8unorm
    @$env:ARW_TEXT_PARITY_DIR = (Resolve-Path target\css-style-parity).Path; $env:ARW_TEXT_PARITY_CHECKPOINTS = "default,compact,hidpi"; $env:ARW_TEXT_PARITY_VISUAL_TIME_MILLIS = "9000"; $env:ARW_TEXT_PARITY_FONT_URL = "./assets/arcweft-demo.ttf"; $env:ARW_TEXT_PARITY_REQUIRED_TEXT = "DSL-styled text|wave motion"; node web\tests\text-parity-smoke.mjs; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo +nightly -Zscript tools\run-text-parity-gates.rs --dir target\css-style-parity --font web\assets\arcweft-demo.ttf --checkpoints default,compact,hidpi

unified-text-visual-parity:
    @New-Item -ItemType Directory -Force -Path web\local,target\unified-text-visual-parity | Out-Null
    @cargo run -p arcweft-cli -- bundle samples/unified-text-visual-parity/main.arcw --output web/local/unified-text-visual-parity.awfb
    @cargo build -p arcweft-player-web --target wasm32-unknown-unknown
    @wasm-bindgen --target web --out-dir web\pkg --out-name arcweft_player_web target\wasm32-unknown-unknown\debug\arcweft_player_web.wasm
    @$cases = @([pscustomobject]@{ Name = "vertical-rl"; Advance = 0; Time = 9000; Scope = $true }, [pscustomobject]@{ Name = "vertical-lr"; Advance = 1; Time = 9000; Scope = $true }, [pscustomobject]@{ Name = "jlreq-loose"; Advance = 2; Time = 9000; Scope = $false }, [pscustomobject]@{ Name = "jlreq-strict"; Advance = 3; Time = 9000; Scope = $false }, [pscustomobject]@{ Name = "fx-4000"; Advance = 4; Time = 4000; Scope = $false }, [pscustomobject]@{ Name = "fx-4500"; Advance = 4; Time = 4500; Scope = $true }, [pscustomobject]@{ Name = "reveal-20000"; Advance = 4; Time = 20000; Scope = $false }, [pscustomobject]@{ Name = "reveal-20500"; Advance = 4; Time = 20500; Scope = $false }); foreach ($case in $cases) { $captureArgs = @("+nightly", "-Zscript", "tools\capture-text-parity-frame.rs", "--bundle", "web\local\unified-text-visual-parity.awfb", "--checkpoint", $case.Name, "--font", "web\assets\arcweft-demo.ttf", "--no-additional-fonts", "--additional-font", "web\assets\noto-sans-jp-unified-text-parity.ttf", "--output", "target\unified-text-visual-parity\native-$($case.Name).png", "--frame-report", "target\unified-text-visual-parity\native-$($case.Name).frame.json", "--viewport", "default", "--advance-count", $case.Advance, "--visual-time-millis", $case.Time, "--target-format", "rgba8unorm"); if ($case.Scope) { $captureArgs += @("--scope-capture-prefix", "target\unified-text-visual-parity\native-$($case.Name).body") }; cargo @captureArgs; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @$env:ARW_TEXT_PARITY_SAMPLE = "unified-text-visual-parity"; $env:ARW_TEXT_PARITY_DIR = (Resolve-Path target\unified-text-visual-parity).Path; $env:ARW_TEXT_PARITY_CHECKPOINTS = "vertical-rl,vertical-lr,jlreq-loose,jlreq-strict,fx-4000,fx-4500,reveal-20000,reveal-20500"; $env:ARW_TEXT_PARITY_BUNDLE_URL = "./local/unified-text-visual-parity.awfb"; $env:ARW_TEXT_PARITY_FONT_URL = "./assets/arcweft-demo.ttf"; $env:ARW_TEXT_PARITY_ADDITIONAL_FONT_URLS = "./assets/noto-sans-jp-unified-text-parity.ttf"; $env:ARW_TEXT_PARITY_VISUAL_TIMES = "vertical-rl=9000,vertical-lr=9000,jlreq-loose=9000,jlreq-strict=9000,fx-4000=4000,fx-4500=4500,reveal-20000=20000,reveal-20500=20500"; $env:ARW_TEXT_PARITY_ADVANCE_COUNTS = "vertical-rl=0,vertical-lr=1,jlreq-loose=2,jlreq-strict=3,fx-4000=4,fx-4500=4,reveal-20000=4,reveal-20500=4"; $env:ARW_TEXT_PARITY_REQUIRED_TEXT = ""; $env:ARW_TEXT_PARITY_REQUIRED_TEXT_BY_CHECKPOINT = "vertical-rl=天地|2026|ARC;vertical-lr=縦|夢|2026|XYZ;jlreq-loose=天地|人山川海;jlreq-strict=天地|人山川海;fx-4000=FX|波動光|REVEAL;fx-4500=FX|波動光|REVEAL;reveal-20000=FX|波動光|REVEAL|開;reveal-20500=FX|波動光|REVEAL|開"; node web\tests\text-parity-smoke.mjs; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    @cargo +nightly -Zscript tools\run-text-parity-gates.rs --dir target\unified-text-visual-parity --font web\assets\arcweft-demo.ttf --checkpoints vertical-rl,vertical-lr,jlreq-loose,jlreq-strict,fx-4000,fx-4500,reveal-20000,reveal-20500
    @foreach ($backend in @("native", "web")) { imq compare "target\unified-text-visual-parity\$backend-jlreq-loose.png" "target\unified-text-visual-parity\$backend-jlreq-strict.png" --metrics psnr,ssim,mse,mae,maxae --format json --output "target\unified-text-visual-parity\imq-$backend-jlreq.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; imq compare "target\unified-text-visual-parity\$backend-fx-4000.png" "target\unified-text-visual-parity\$backend-fx-4500.png" --metrics psnr,ssim,mse,mae,maxae --format json --output "target\unified-text-visual-parity\imq-$backend-fx.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; imq compare "target\unified-text-visual-parity\$backend-reveal-20000.png" "target\unified-text-visual-parity\$backend-reveal-20500.png" --metrics psnr,ssim,mse,mae,maxae --format json --output "target\unified-text-visual-parity\imq-$backend-reveal.json"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @cargo +nightly -Zscript tools\verify-unified-text-visual-parity.rs --dir target\unified-text-visual-parity

vertical-writing-style-sample:
    @New-Item -ItemType Directory -Force -Path target\vertical-writing-style | Out-Null
    @cargo run -p arcweft-cli -- check samples\vertical-writing-style\main.arcw
    @cargo run -p arcweft-cli -- bundle samples\vertical-writing-style\main.arcw --output target\vertical-writing-style\vertical-writing-style.awfb
    @$cases = @([pscustomobject]@{ Name = "vertical-rl"; Advance = 0 }, [pscustomobject]@{ Name = "vertical-lr"; Advance = 1 }, [pscustomobject]@{ Name = "jlreq-loose"; Advance = 2 }, [pscustomobject]@{ Name = "jlreq-strict"; Advance = 3 }); foreach ($case in $cases) { cargo +nightly -Zscript tools\capture-text-parity-frame.rs --bundle target\vertical-writing-style\vertical-writing-style.awfb --checkpoint $case.Name --font web\assets\arcweft-demo.ttf --no-additional-fonts --additional-font web\assets\noto-sans-jp-unified-text-parity.ttf --output "target\vertical-writing-style\native-$($case.Name).png" --frame-report "target\vertical-writing-style\native-$($case.Name).frame.json" --viewport default --advance-count $case.Advance --visual-time-millis 9000 --target-format rgba8unorm; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
    @Add-Type -AssemblyName System.Drawing; foreach ($name in @("vertical-rl", "vertical-lr", "jlreq-loose", "jlreq-strict")) { $bitmap = [System.Drawing.Bitmap]::FromFile((Resolve-Path "target\vertical-writing-style\native-$name.png")); try { $panel = $bitmap.GetPixel(150, 180); if ($panel.R -ne 69 -or $panel.G -ne 81 -or $panel.B -ne 101 -or $panel.A -ne 255) { throw "$name did not render the authored panel background" } } finally { $bitmap.Dispose() } }
    @$expectedModes = @{ "vertical-rl" = "vertical_rl"; "vertical-lr" = "vertical_lr"; "jlreq-loose" = "vertical_rl"; "jlreq-strict" = "vertical_rl" }; $expectedRuby = @{ "vertical-rl" = 1; "vertical-lr" = 2 }; foreach ($name in $expectedModes.Keys) { $report = Get-Content "target\vertical-writing-style\native-$name.frame.json" -Encoding UTF8 -Raw | ConvertFrom-Json; $speaker = @($report.text | Where-Object { $_.owner.kind -like "view:*" })[0]; $content = @($report.text | Where-Object { $_.owner.kind -like "dialogue:*" })[0]; $modes = @($content.lines | ForEach-Object { $_.writing_mode } | Select-Object -Unique); if ($modes -notcontains $expectedModes[$name]) { throw "$name did not observe writing mode $($expectedModes[$name])" }; if (($speaker.glyphs[0].rgba -join ",") -ne "139,211,255,255" -or $speaker.runs[0].style.font_size_milli -ne 18000 -or $speaker.runs[0].style.font_families -notcontains "Noto Sans JP") { throw "$name did not apply the speaker style part" }; if (($content.glyphs[0].rgba -join ",") -ne "244,248,255,255" -or $content.runs[0].style.font_families -notcontains "Noto Sans JP") { throw "$name did not apply the content style part" }; if ($expectedRuby.ContainsKey($name) -and ($content.ruby_count -ne $expectedRuby[$name] -or @($content.glyphs | Where-Object { $_.orientation -eq "text_combine_upright" }).Count -eq 0)) { throw "$name did not preserve ruby and text-combine-upright observations" } }
    @cargo +nightly -Zscript tools\capture-text-parity-frame.rs --bundle target\vertical-writing-style\vertical-writing-style.awfb --checkpoint vertical-rl-repeat --font web\assets\arcweft-demo.ttf --no-additional-fonts --additional-font web\assets\noto-sans-jp-unified-text-parity.ttf --output target\vertical-writing-style\native-vertical-rl-repeat.png --frame-report target\vertical-writing-style\native-vertical-rl-repeat.frame.json --viewport default --advance-count 0 --visual-time-millis 9000 --target-format rgba8unorm
    @$hash = { param($path) $sha = [System.Security.Cryptography.SHA256]::Create(); try { [System.BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($path))).Replace("-", "") } finally { $sha.Dispose() } }; $first = & $hash target\vertical-writing-style\native-vertical-rl.png; $repeat = & $hash target\vertical-writing-style\native-vertical-rl-repeat.png; if ($first -ne $repeat) { throw "vertical-writing-style repeated capture was not pixel deterministic" }; Write-Host "vertical-writing-style repeated capture SHA-256: $first"
    @$hash = { param($path) $sha = [System.Security.Cryptography.SHA256]::Create(); try { [System.BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($path))).Replace("-", "") } finally { $sha.Dispose() } }; $loose = & $hash target\vertical-writing-style\native-jlreq-loose.png; $strict = & $hash target\vertical-writing-style\native-jlreq-strict.png; if ($loose -eq $strict) { throw "vertical-writing-style loose and strict JLREQ captures were identical" }; Write-Host "vertical-writing-style observed styled parts, vertical writing modes, and distinct JLREQ frames"

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
