# ADR-003: text layout は Sans I/O、glyphon 接続は adapter crate に置く

## Status

Proposed

## Context

Arcweft は core/data-format の Sans I/O と adapter boundary を重視します。text layout は deterministic な runtime/presentation data に近く、GPU resource creation とは分離すべきです。

## Decision

```text
arcweft-render-text      = authored/resolved rich-text sidecar
arcweft-text-layout      = Sans I/O layout engine
glyphon-layout-ext-api   = renderer extension API shape
arcweft-glyphon          = glyphon/wgpu adapter
```

`arcweft-text-layout` は glyphon/wgpu/filesystem/window に依存しません。

## Consequences

良い点:

```text
- headless layout test が容易。
- Agent observation を GPU なしで生成できる。
- native/web renderer を差し替えられる。
```

悪い点:

```text
- adapter crate で cache key/lifetime 変換が必要。
- renderer と layout の共同テスト fixture が別途必要。
```
