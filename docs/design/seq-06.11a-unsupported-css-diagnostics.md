# Unsupported CSS diagnostics

## Supported values in this cut

- `background-color` with resolved RGBA color.
- Uniform circular `border-radius` after Takumi computation.
- Uniform visible solid border across physical sides.
- `background-image: linear-gradient(...)` when non-repeating and color stops can
  be represented as normalized `ViewGradientStop` values.
- `background-image: url(...)` when a resource index is already available in the
  adapter/player-provided resource table.
- `opacity` as a paint-only field on `DirectBoxPaint`.

## Diagnosed values

The extractor emits `TakumiDiagnosticCode::UnsupportedDirectCss` for:

- radial gradients;
- conic gradients;
- repeating linear gradients;
- gradient color hints without a color stop;
- gradient stop positions that cannot be normalized without layout-dependent
  axis data;
- mixed or elliptical border radii;
- non-solid visible border styles;
- mixed visible border widths;
- mixed visible border colors;
- image URLs not present in the resource table;
- background layers that depend on unsupported repeat/position/size semantics.

## Preservation policy

When an unsupported layer appears alongside supported layers, the extractor keeps
supported layers and emits diagnostics for unsupported layers. It never silently
falls back to:

- browser CSS;
- DOM or hidden HTML controls;
- canvas 2D;
- Takumi CPU raster output;
- screenshots or image snapshots.

## Resource requirement policy

A missing image URL is not a paint extraction failure. It produces:

```rust
DirectPaintResourceRequirement {
    path,
    url,
}
```

The adapter/player resource layer may satisfy that requirement and re-run paint
extraction with an updated `DirectPaintResourceTable`.
