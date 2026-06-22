# Font asset required by the WebGPU MVP

Place a project-owned, license-compatible TTF or OTF at:

```text
assets/arcweft-demo.ttf
```

The same bytes should be registered through `SharedRenderer::register_font_bytes`
in native and browser visual-parity jobs. This package intentionally contains no
font binary.
