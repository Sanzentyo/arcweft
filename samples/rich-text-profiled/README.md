# rich-text-profiled

`rich-text-profiled` is a compact project-shaped sample for launch-profile
selection of `dialogue defaults`.

The same source file declares an ID-free desktop defaults profile and a named
mobile profile. The desktop launch profile omits `dialogue_defaults`; the
mobile launch profile selects `dialogue.mobile`. Runtime-plan JSON and LSP
cascade features can therefore point at the selected defaults block.

## Useful commands

```bash
cargo run -p arcweft-cli -- check --manifest samples/rich-text-profiled/arcw.toml --profile desktop
cargo run -p arcweft-cli -- check --manifest samples/rich-text-profiled/arcw.toml --profile mobile
cargo run -p arcweft-cli -- plan --manifest samples/rich-text-profiled/arcw.toml --profile mobile --json
cargo run -p arcweft-cli -- agent observe --manifest samples/rich-text-profiled/arcw.toml --profile mobile --json --image png --out target/rich-text-profiled-mobile.png --mode drain --steps 4 --max-ops 128
cargo run -p arcweft-cli -- agent observe --manifest samples/rich-text-profiled/arcw.toml --profile mobile --json --image png --layer dialogue --out target/rich-text-profiled-dialogue.png --mode drain --steps 4 --max-ops 128
cargo run -p arcweft-cli -- fmt --canonical-rich-text samples/rich-text-profiled/src/main.arcw
```
