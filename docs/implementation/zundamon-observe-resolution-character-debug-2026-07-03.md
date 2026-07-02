# Zundamon observe resolution and character package debug

Date: 2026-07-03

## Summary

`samples/zundamon-stand-switch/main.arcw` renders correctly through
player-backed `agent observe`, and the rendered PNG and observe JSON now both
expose the standing Zundamon image object.

The remaining visual issue at larger viewport sizes is not an observe/renderer
metadata mismatch. It is an authoring/runtime layout limitation: the sample
declares the standing image with absolute pixel coordinates and dimensions:

```arcw
x = 930px
y = 20px
width = 250px
height = 430px
fit = "contain"
```

When the viewport grows, the dialogue panel follows the viewport width, while
the standing image remains at the same absolute position and size.

## Evidence

The following commands were run from the repository root:

```bash
cargo run -p arcweft-cli --bin arcw -- agent observe samples/zundamon-stand-switch/main.arcw --steps 2 --image png --capture color --content-policy-mode local-dev --out target/zundamon-debug/zundamon-agent-final.png --json
cargo run -p arcweft-cli --bin arcw -- agent observe samples/zundamon-stand-switch/main.arcw --steps 2 --viewport-width 1920 --viewport-height 1080 --image png --capture color --content-policy-mode local-dev --out target/zundamon-debug/zundamon-agent-1920x1080.png --json
cargo run -p arcweft-cli --bin arcw -- agent observe samples/zundamon-stand-switch/main.arcw --steps 2 --viewport-width 2560 --viewport-height 1440 --image png --capture color --content-policy-mode local-dev --out target/zundamon-debug/zundamon-agent-2560x1440.png --json
```

Observed geometry:

| Viewport | Dialogue bbox | Character layer bbox |
| --- | --- | --- |
| 1280x720 | `96,548,1088,124` | `976,20,158,430` |
| 1920x1080 | `96,908,1728,124` | `976,20,158,430` |
| 2560x1440 | `96,1268,2368,124` | `976,20,158,430` |

The character object appears in observe JSON as:

```json
{
  "id": "object.image.image.zundamon.normal",
  "layer": "layer.character",
  "target": "target.zundamon.stand",
  "asset": "asset.zundamon.normal",
  "fit": "contain"
}
```

## Character asset finding

The Zundamon sample currently uses `tools/prepare-zundamon-sample.rs` to import
the PSD into Arcweft's typed character model, but then it composes only two flat
PNG files:

- `normal.png`
- `smile.png`

The generated local files in this checkout have different alpha-cropped
dimensions:

| File | Dimensions |
| --- | --- |
| `.arcweft/asset/zundamon/normal.png` | `522x1425` |
| `.arcweft/asset/zundamon/smile.png` | `776x1425` |

Both are then placed into the same 250x430 authored image bounds using
`contain` and center/bottom alignment. This preserves the bounding box center
but does not preserve source-canvas part locations. It is therefore expected
that pose/expression changes can appear to shift.

This is useful as a smoke sample but is not the correct final product path for
standing characters. Flattened PNGs lose:

- PSD layer identity;
- part/variant identity;
- source rectangles;
- source canvas anchor;
- z ordering beyond the baked pixels;
- pose/expression typed selections;
- stable LSP-completable look/part/variant symbols.

The repository already has the correct substrate:

- `arcweft-character` owns `arcweft.character` v1 `.awchar` manifests.
- `arcweft-character-psd` imports PSD bytes into `.awchar` package data without
  calling PSD flattening.
- `arcweft-presentation::character::CharacterRenderSpec` resolves typed looks
  into deterministic layer stacks.
- `arcweft-character-ui` lowers `CharacterRenderSpec` into retained UI layers.
- `docs/schemas/character-manifest.md` documents the `.awchar` format.

The missing work is production integration: `.awchar` needs to become a normal
bundle/player/LSP-backed character-stage resource instead of a side substrate
that sample tooling flattens into PNGs.

## Additional route finding

`tools/capture-bundle-scene-frame.rs` currently reads a JSON bundle with
`ArcweftBundle::from_json_slice`. It fails if pointed at an AWFB binary produced
by the default `arcw bundle --format awfb` path. For this script, use:

```bash
cargo run -p arcweft-cli --bin arcw -- bundle samples/zundamon-stand-switch/main.arcw --format json --output target/zundamon-debug/zundamon-stand-switch.bundle.json
cargo +nightly -Zscript tools/capture-bundle-scene-frame.rs target/zundamon-debug/zundamon-stand-switch.bundle.json --output target/zundamon-debug/zundamon-native-normal-1280.png --width 1280 --height 720
```

This is a script/README mismatch rather than the primary character rendering
issue. The `.awchar` integration request should decide whether this script is
updated to consume AWFB or whether its README examples consistently use JSON
bundle output.

## Follow-up requests

- `docs/reviews/requests/2026-07-03-seq-06.14-responsive-stage-placement-package.md`
- `docs/reviews/requests/2026-07-03-seq-06.15-awchar-character-stage-runtime-lsp-package.md`

Seq06.14 owns responsive placement and large-viewport evidence. Seq06.15 owns
typed `.awchar` character-stage integration and Zundamon sample migration.
