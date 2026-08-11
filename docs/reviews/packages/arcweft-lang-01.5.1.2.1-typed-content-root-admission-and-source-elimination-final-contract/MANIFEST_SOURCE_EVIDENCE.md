# Manifest source evidence

## 1. Sole source-map owner

`arcweft-launch::ManifestSourceMap` remains the only map from strict schema-1
syntax to revision-bound `SourceSpan`. This contract extends its existing
`ManifestTokenPath` owner; it does not add a content map.

## 2. Required token paths

For each selected content unit the admission coordinator requests:

- unit table;
- each root array element by zero-based ordinal;
- visibility value;
- demand value;
- selected profile-content table;
- residency value;
- placement value;
- compression value.

The existing internal path representation already has content-unit,
profile-content, field, and index segments. The new public token variants map
to those segments through the enum's owning inherent implementation.

## 3. Span validation

Every returned span SHALL:

- belong to the exact accepted manifest `SourceDocumentIdentity`;
- be in bounds for that document;
- identify the value/token selected by the strict decode;
- remain distinct for duplicate-first/duplicate-later diagnostics where the
  decoder rejects the document; and
- never be reconstructed by searching TOML text.

A successful manifest decode with missing source evidence is an internal
candidate error, not permission to synthesize a span.

## 4. Diagnostic ownership

| Diagnostic | Primary range | Related ranges |
| --- | --- | --- |
| unknown/ambiguous/invisible/wrong-family root | exact root array element | candidate declarations or visibility boundary |
| required missing | exact root array element | demand value, expected logical path |
| optional referenced missing | exact root array element | demand value, every bounded reference range, expected logical path |
| invalid profile policy | exact field value | content-unit table and profile-content table |
| topology/source revision mismatch | requesting overlay/document range where available | accepted manifest identity/revision |

LSP converts `SourceSpan` to URI/range only at the adapter boundary. Project,
character, manifest, and sema types remain URI-free.
