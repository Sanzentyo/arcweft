# rich-text-profiled

`rich-text-profiled` is a compact project-shaped sample for launch-profile
selection of `dialogue defaults`.

The same source file declares a canonical desktop defaults profile and a mobile
profile. `arcw.toml` selects the active profile through `dialogue_defaults`, so
runtime-plan JSON and LSP cascade features can point at the selected defaults
block instead of always using `@dialogue.defaults`.

## Useful commands

```bash
cargo run -p arcweft-cli -- check --manifest samples/rich-text-profiled/arcw.toml --profile desktop
cargo run -p arcweft-cli -- check --manifest samples/rich-text-profiled/arcw.toml --profile mobile
cargo run -p arcweft-cli -- plan --manifest samples/rich-text-profiled/arcw.toml --profile mobile --json
cargo run -p arcweft-cli -- agent observe --manifest samples/rich-text-profiled/arcw.toml --profile mobile --json --image png --out target/rich-text-profiled-mobile.png --mode drain --steps 4 --max-ops 128
cargo run -p arcweft-cli -- agent observe --manifest samples/rich-text-profiled/arcw.toml --profile mobile --json --image png --layer dialogue --out target/rich-text-profiled-dialogue.png --mode drain --steps 4 --max-ops 128
cargo run -p arcweft-cli -- fmt --canonical-rich-text samples/rich-text-profiled/src/main.arcw
```
