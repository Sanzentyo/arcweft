# Packaging and product flags

## Feature flags

```toml
[features]
default = ["vm", "wgpu-render", "audio-basic"]

native = ["native-st", "wgpu-render", "audio-native"]
native-mt = ["tokio", "rayon"]
native-jit = ["cranelift"]
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

```text
game.awfb
  manifest.toml
  program.ir
  bytecode.vm
  graph.index
  entities.toml
  assets/
  shaders/
  ui/
  audio/
  text/
  source_maps/
  contracts/
```

## Debug bundle

```text
bug-report/
  engine.json
  bundle-manifest.toml
  state-before.bin
  trace.awftx
  agent-actions.ndjson
  logs.jsonl
  signals.json
  metrics.json
  screenshot.png
  overlay.png
  audio-state.json
  diagnostics.json
```

