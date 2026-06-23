# CLI Run Project Manifest Cut

This cut makes `arcw.toml` the project-level entry point for `arcw run`.

## Current behavior

- `arcw run <path.arcw>` remains a direct single-source development route.
- `arcw run --profile <id>` resolves `<id>` from `arcw.toml`.
- `arcw run` with no path/profile reads `arcw.toml`, then selects:
  - the root `default = "..."` profile when present;
  - otherwise the only `kind = "game"` profile;
  - otherwise the only profile in the manifest.
- Native is the default interactive runner for game profiles when the CLI is
  built with the native-player feature.
- `--runner headless` selects the deterministic step/debug runner.
- `--runner web` updates the ignored browser-player bundle under `web/local/`.

## Source graph boundary

The current project loader still loads one root `.arcw` source file per launch
profile. Manifest-managed side inputs such as character manifests, adapter
manifests, Rust ABI metadata, and assets are multi-file, but physical `.arcw`
module graph loading is not implemented in this cut.

`mod` and Rust-like module paths are language-surface concepts today. They are
parsed and lowered as source declarations inside the loaded root source; they do
not yet cause sibling `.arcw` files to be discovered from the filesystem.

## Wasm boundary

The web runner currently refreshes the bundle consumed by the existing web
player. Building the wasm player remains the ordinary Cargo/`wasm-bindgen`
route, using Cargo's `--target wasm32-unknown-unknown` in its existing sense.
