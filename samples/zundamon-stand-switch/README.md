# Zundamon Stand Switch Sample

This sample demonstrates switching one standing Zundamon image object between
normal and smile surfaces, with separate Zundamon and narrator dialogue styles.
The generated PNGs are derived from a PSD fixture and are intentionally not
tracked by the repository.

Source material:

- https://seiga.nicovideo.jp/seiga/im11206626
- Expected local PSD default:
  `.arcweft-local/character-psd/zundamon-v3.2/ずんだもん立ち絵素材V3.2_全部詰め版.psd`

Prepare the local sample assets:

```bash
cargo +nightly -Zscript tools/prepare-zundamon-sample.rs --apply
```

Override the source path when needed:

```bash
cargo +nightly -Zscript tools/prepare-zundamon-sample.rs --apply --source "<local-psd-path>"
```

The script writes only ignored files under:

```text
samples/zundamon-stand-switch/assets/zundamon/
```

Run through the CLI routes after preparing assets:

```bash
cargo run -p arcweft-cli --bin arcw -- compile samples/zundamon-stand-switch/src/main.arcw --emit check
cargo run -p arcweft-cli --bin arcw -- run samples/zundamon-stand-switch/src/main.arcw
cargo run -p arcweft-cli --bin arcw -- run samples/zundamon-stand-switch/src/main.arcw --runner headless --steps 2 --mode drain --max-ops 32 --json
cargo run -p arcweft-cli --bin arcw -- agent observe samples/zundamon-stand-switch/src/main.arcw --steps 2 --image png --capture color --content-policy-mode local-dev --out target/zundamon-agent.png --json
```

Inside this sample directory, the same profile run is just:

```bash
cargo run -p arcweft-cli --bin arcw -- run
```

Capture the shared native/WebGPU scene, including presentation images:

```bash
cargo run -p arcweft-cli --bin arcw -- bundle samples/zundamon-stand-switch/src/main.arcw --output target/zundamon-stand-switch.awfb
cargo +nightly -Zscript tools/capture-bundle-scene-frame.rs target/zundamon-stand-switch.awfb --output target/zundamon-native-scene.png
cargo +nightly -Zscript tools/capture-bundle-scene-frame.rs target/zundamon-stand-switch.awfb --output target/zundamon-native-smile.png --select-choice choice.zundamon.to_smile
```

For the browser player, update the ignored `web/local/` bundle with:

```bash
cargo run -p arcweft-cli --bin arcw -- run samples/zundamon-stand-switch/src/main.arcw --runner web
```

Then open `web/index.html?bundle=./local/zundamon-stand-switch.awfb` after the
normal wasm build step. The bundle copy under `web/local/` is ignored.
