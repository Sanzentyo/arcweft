# Character Manifest (`.awchar`)

`arcweft.character` version 1 is the typed, renderer-independent description of
one composited standing character. An `.awchar` package is a directory containing
`character.awchar.json` and package-relative PNG resources.

The Rust owner is `arcweft-character`. It validates identifiers, paths, complete
look selections, duplicate parts/variants/assets, the default look, and deterministic
bottom-to-top resolution. File I/O remains in adapter crates.

The normative machine-readable shape is
[`character-awchar.schema.json`](character-awchar.schema.json). Rust validation is
stricter than JSON Schema where uniqueness depends on object keys rather than whole
JSON values.

## Resource identity

A variant path is package-relative. Presentation derives a stable resource id:

```text
asset.character.{character-suffix}.{part}.{variant}
```

For example, `character.akane`, part `eyes`, variant `smile` becomes
`asset.character.akane.eyes.smile`. This lets UI/view resource nodes and the
presentation layer tree refer to the same typed resource without serializing a
redundant id into every manifest.

## Look semantics

Every look selects exactly one variant from every declared part. Switching a look
therefore creates a total, deterministic composition; renderers do not infer state
from layer visibility or file names at runtime.
