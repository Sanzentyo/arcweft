# Packaging and product flags

Packaging is split into two layers:

- Bundle format crates define Sans I/O data structures and deterministic codecs over `&[u8]`, `Vec<u8>`, and manifest strings.
- CLI/build/player adapters perform filesystem reads/writes, embedding, compression selection, signing, upload, and platform storage.

`.awfb` is a portable data artifact. Opening a path, watching a directory, fetching a remote bundle, or writing a crash report is never part of `arcweft-core` or the bundle data model.

## Feature flags

```toml
[features]
default = ["vm", "wgpu-render", "audio-basic"]

native = ["native-st", "wgpu-render", "audio-native"]
native-mt = ["tokio", "rayon"]
native-jit = ["arcweft-lang-jit-cranelift"]
web = ["web-st", "dom-ui", "audio-web"]
web-mt = ["web-workers", "wasm-bindgen-rayon"]

agent-observe = []
agent-control = ["agent-observe"]
agent-debug-mutate = ["agent-control"]
agent-mcp = ["agent-observe"]

servo-ui = []
dom-ui = []
```

## Runtime flags

```text
--agent=off|observe|control|debug
--observe=off|metrics|logs|agent|debug
--debug-assertions=on|off
--audio-backend=native|web|dummy
--headless
```

## Bundle contents

Current CLI bundle slice:

```text
game.awfb
  schema_version
  manifest
    source_label
    profile_id
    profile_kind
    entry
    adapter
    adapter_manifest_ids
    required_host_calls
    runtime
      entry_flow
      flows
      bytecode_instructions
      line_task_groups
      stream_plans
      source_plans
  source
    label
    text
  bytecode
    encoding = structured_json
    program
      entry_flow
      entries
      flows
      pure_helpers
      line_task_groups
      stream_plans
      source_plans
  adapter_manifests[]
    id
    display_name
    effects
    host_calls[]
      id
      effects
  virtual_files[]
    space
    path
    bytes
```

The implemented `.awfb` codec is deterministic JSON owned by
`arcweft-bundle`. The crate performs no filesystem, clock, network, signing, or
compression work. CLI/build/player adapters are responsible for turning source
trees and virtual file roots into bundle values, and for materializing bundle
values into a runnable host workspace. `arcw run-bundle` executes the decoded
bytecode section directly and does not parse, typecheck, or lower the source
text again.

The CLI includes `.arcweft/asset` by default and can opt into `.arcweft/save`,
`.arcweft/temp`, and `.arcweft/export`. Packaged virtual paths use only normal
relative components. Parent traversal, absolute paths, and host path prefixes
are rejected or omitted before encoding.

Future product bundle slices can replace structured JSON bytecode with a
compact deterministic binary bytecode section and add graph indexes, entity
tables, source maps, contracts, shaders, UI, audio, and text resources as typed
bundle sections:

```text
game.awfb
  manifest
  bytecode.vm
  graph.index
  entities
  assets
  shaders
  ui
  audio
  text
  source_maps
  contracts
```

## Debug bundle

```text
bug-report/
  engine.json
  bundle-manifest.toml
  state-before.bin
  trace.arcwx
  agent-actions.ndjson
  logs.jsonl
  signals.json
  metrics.json
  screenshot.png
  overlay.png
  audio-state.json
  diagnostics.json
```
