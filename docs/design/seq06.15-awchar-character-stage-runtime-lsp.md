# seq06.15 `.awchar` character-stage runtime and LSP design

## Grounding

Arcweft already has the right substrate for this sequence:

- `arcweft-character` owns the Sans I/O typed manifest, identifiers, validation, catalog, and deterministic look resolution.
- `arcweft-character-psd` accepts PSD bytes and returns a manifest plus package-relative PNG payloads without filesystem I/O.
- `arcweft-presentation::character::CharacterRenderSpec` is the renderer-independent truth for a selected look.
- `arcweft-character-view` lowers a resolved character to retained View image nodes, layouts, image sources, and View layer output with no browser DOM dependency.
- launch profiles already declare `character_manifests`, and LSP profile loading builds a `CharacterCatalog` from those entries.

The production cut therefore wires those pieces together instead of introducing a flat-PNG compatibility path.

## Product and bundle representation

An `.awchar` package is a directory resource.  The canonical package layout is:

```text
<id>.awchar/
  character.awchar.json
  layers/<part>--<variant>.png
```

The manifest is stored as deterministic UTF-8 JSON bytes.  Layer payloads remain package-relative PNGs referenced from manifest variants.  Runtime and tooling must validate that every referenced layer payload is present and that no unreferenced payload is silently published.

The new `arcweft-character::package` module owns the Sans I/O package invariant:

- `CharacterPackage` = validated `CharacterManifest` + exact manifest bytes + ordered layer payloads;
- `CharacterLayerPayload` = package-relative `CharacterAssetPath` + bytes;
- `CharacterPackageError` = missing, duplicate, or unreferenced payloads plus manifest codec errors.

Bundle publication adds a typed `BundleCharacterPackage` resource under `arcweft-bundle`, backed by normal `BundleVirtualFile` entries in `BundleVirtualFileSpace::Asset`.  AWFB and AWFR publication carry the same resource table as structured bundle data; external-payload placement and compression are inherited from existing AWFB/AWFR virtual-file sections rather than inventing a character-specific container.

Bundle schema is incremented from `4` to `5` because `ArcweftBundle` gains a serialized `character_packages` field.  A bundle is invalid when a character package points at a missing manifest file or missing layer file.  Missing layer payloads are rejected before player decode.

## Source authoring

A project exposes character packages through launch profile metadata:

```toml
default = "dev"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
character_manifests = ["assets/zundamon.awchar"]
```

Arcweft source continues to use normal presentation syntax:

```arcw
show(@character.zundamon, look = .normal)
show(@character.zundamon, look = .smile)
```

The `look` argument is a per-character enum type registered from loaded manifests, so `.normal` resolves against `CharacterLook<character.zundamon>`.  `pose`, `expression`, `mouth`, and other axes remain manifest parts/variants.  A named look is a total selection of one variant per declared part; authors can expose user-friendly looks while preserving lower-level part/variant metadata for tooling.

`character.stage.show(...)` should lower to the same runtime operation as `show(..., look = ...)`: resolve the selected manifest look, build a `CharacterRenderSpec`, then stage a `CharacterSurface::from_render_spec`.  It is not an image-object API and must not synthesize a flattened asset.

## Runtime and player representation

`CharacterRenderSpec` is the runtime truth for a selected character look.  The overlay extends it with:

- copied `CharacterSourceLayer` metadata per render layer;
- `CharacterStageBounds`, computed from the source canvas anchor as `(-anchor.x, -anchor.y, canvas.width, canvas.height)`;
- typed `CharacterRenderDiagnostic` values for unsupported blend modes and clipping;
- deterministic layer order from manifest `z` and part id.

This makes expression or pose switching stable: the stage-space bbox comes from the source canvas and anchor, not from the cropped layer extents.  Cropped PNG dimensions still match each layer source rectangle exactly.

Unsupported blend and clipping behavior is never silently approximated.  Importers preserve metadata, `CharacterRenderSpec::diagnostics()` reports baseline renderer support gaps, and retained-View lowering can run in either strict or metadata-preserving mode.

## Renderer path

Native and web players use the same prepared-frame path:

```text
CharacterPackage bytes
  -> CharacterManifest + package-relative PNG payloads
  -> CharacterRenderSpec
  -> CharacterViewView / layer frames
  -> shared wgpu image submission
```

The retained View path is the first production lowering because it already shares layout, image decode, animation frame resolution, and View layer output across platforms.  Browser DOM is not involved.  There is no runtime fallback that pre-flattens `normal.png` or `smile.png`.

Seq06.14 placement is used at the stage-object boundary: the whole character bbox is placed as one object, and the layer rectangles remain in source-canvas coordinates below that stable root.

## Agent observe output

Observe emits one character object with stable metadata:

```json
{
  "kind": "character",
  "id": "character.zundamon",
  "look": "smile",
  "bbox": { "x": -48, "y": -128, "width": 96, "height": 128 },
  "capture_ref": "capture.character.zundamon",
  "layers": [
    {
      "part": "eyes",
      "variant": "smile",
      "asset": "layers/eyes--smile.png",
      "rect": { "x": 34, "y": 48, "width": 28, "height": 8 },
      "z": 10,
      "source_layer": "part:eyes/smile",
      "capture_ref": "capture.character.zundamon.eyes.smile"
    }
  ]
}
```

Layer capture refs are optional at renderer capability level but deterministic when emitted.  The whole-character capture ref is always available for observe surfaces.

## LSP and tooling

LSP profile loading already reads `character_manifests`.  Completion and hover extend that profile data as follows:

- character id completion: `@character.zundamon`;
- look completion: `.normal`, `.smile` with selected part/variant summary;
- part completion: `.eyes`, `.mouth`;
- variant completion: `.smile`, `.neutral` with PSD source group/layer names where present;
- hover for `@character.*`, `.look`, `.part`, and `.variant` tokens uses loaded manifest data;
- diagnostics for missing/invalid manifests continue to surface as `profile.character_manifest.*` diagnostics.

The generic enum-short completion path remains useful, but character metadata completions add richer detail and source-layer documentation.

## Import tooling

`arcw import psd-character` remains the production importer.  It writes a `.awchar` directory with `character.awchar.json` plus layer PNG payloads.  PSD groups use the contract:

```text
part:<part-id>      # direct pixel layers become variants
look:<look-id>      # direct marker layers named part=variant define total looks
```

Loose legacy group inference is retained only as importer compatibility diagnostics for existing PSD material; the runtime path is still typed `.awchar` and not flat PNG swapping.

## Deferred PSD feature policy

Deferred features are explicit diagnostics or preserved metadata, not silent approximations:

- blend modes beyond pass-through/normal/multiply/screen in baseline renderer;
- clipping masks in retained View;
- full Photoshop group compositing semantics;
- live PSD editing;
- runtime PSD parsing.
