# Lang-01.5.1.2 — typed content-root admission contract correction

## Sequence position

This is the second correction split from Lang-01.5.1. It follows the
single-manifest decoder and the Lang-01.5.1.1 dialogue presentation-owner
correction. It may be designed while those cuts are implemented, but its
production migration must consume their final typed products.

## Why this split is required

The Lang-01.5.1 package is concrete for schema-1 decoding, profile resolution,
generated metadata, and the deterministic mapping from
`@character.a.b` to `assets/a/b.awchar`. Its test matrix additionally requires:

- rejection when a selected `.awchar` layer payload is missing;
- an explicit fact for an absent, unreferenced optional file-backed root;
- a distinct failure when an absent optional root is referenced;
- source-owned content-root injection into project semantic facts;
- structured rejection of unknown or wrong root families; and
- a topology revision covering all resulting character/content resources.

Those requirements do not have a compatible final carrier in the current
production model:

- `LoadedProfileTopologyResource` retains UTF-8 `SourceDocument` values only;
- editor topology overlays carry `Arc<str>` and cannot represent PNG bytes;
- the loader currently retains a character manifest but does not construct the
  existing Sans-I/O `CharacterPackage` from its layer payloads;
- `SourceSetRevision` is defined over `SourceDocumentIdentity`, not binary
  resource identities;
- no typed optional-absence fact or consumer contract exists;
- source `content` declarations are being removed, but the exact
  manifest-to-`ProjectIndex` content fact has not been specified.

Implementing the remaining matrix rows by guessing would either omit binary
payload identity, create a second resource revision system accidentally, or
preserve the source declaration that the user has directed us to delete.

## Decisions required

1. Define the accepted topology carrier for a complete `.awchar` package.
   Decide whether it retains:
   - one `CharacterPackage`;
   - manifest plus typed binary layer resources; or
   - a package descriptor plus a separately owned content-addressed blob set.
2. Define binary overlay ownership. Specify how an editor/host supplies exact
   layer bytes without coercing binary data into `SourceDocument` or `Arc<str>`.
3. Define the one revision contract covering manifest text, source modules,
   generated metadata, character manifests, layer payloads, and overlays.
   Reuse an existing typed resource/content digest where possible; do not add a
   parallel ad hoc hash merely for this migration.
4. Define `required` and `optional` content-unit admission precisely:
   - when an absent optional root produces an accepted absence fact;
   - what makes that root referenced by the selected profile/runtime;
   - the exact typed error for referenced absence; and
   - how present-but-invalid optional content remains fail-closed.
5. Define the closed root-family model. State which families are file-backed,
   which are source-owned semantic identities, and which families are invalid
   in each content-unit position.
6. Define the manifest-owned content facts injected into `ProjectIndex` after
   source `content` declaration removal, including identity, visibility,
   demand, profile policy, source range, and accepted topology revision.
7. Define bundle/watch/LSP consumers of package payload and optional-absence
   facts so they do not rescan directories or reparse the manifest.

## Required implementation order

1. Finalize typed binary resource and revision ownership.
2. Add binary overlay/admission input without changing the schema-1 decoder.
3. Construct and validate complete character packages at the topology
   transaction boundary.
4. Add required/optional presence facts and family validation.
5. Inject accepted manifest content facts into `ProjectIndex`.
6. Delete source `content` syntax/HIR/sema/tooling ownership atomically.
7. Migrate bundle, watch, LSP, and maintained fixtures.
8. Run focused, workspace, Tier 2, and structural validation.

## Tests the contract must specify

- selected character manifest present with every exact layer payload;
- missing manifest, missing layer, duplicate layer, unreferenced layer, corrupt
  PNG/package, and mismatched character identity;
- nested `@character.npc.alice` path mapping;
- absent optional root when unreferenced, including its explicit typed absence
  fact;
- absent optional root when referenced, with the exact structured diagnostic;
- present optional root with corrupt content, proving optional does not mask
  failure;
- source-owned root injected from the accepted manifest without filesystem
  scanning or source `content` syntax;
- unknown and wrong root families;
- disk and overlay forms for manifest text and binary layer bytes;
- topology/resource revision changes for each manifest, source, metadata,
  character manifest, layer payload, and overlay mutation;
- failed candidate construction publishes no partial topology, project index,
  catalog, cache namespace, or LSP generation;
- bundle/watch inventories consume the same accepted typed resource set.

## Constraints

- Do not redesign or duplicate the accepted strict Taplo decoder,
  `SourceBackedManifest`, generated metadata admission, project containment, or
  Character nominal identity work unless concrete evidence shows a flaw.
- Keep binary bytes out of `SourceDocument` and text-only overlay types.
- Do not scan a directory to infer content not named by the accepted typed
  package contract.
- Do not restore source `content` declarations, old manifest path arrays,
  compatibility readers, aliases, or last-known-good candidate acceptance.
- Keep `arcweft-core` and data-format crates Sans I/O.
- Test behavior through typed APIs; do not introduce source-spelling gates.

## Expected output

Return an implementation-ready final contract containing:

- final Rust shapes and crate ownership;
- dependency direction and Sans-I/O boundaries;
- exact binary overlay and resource-revision rules;
- required/optional presence semantics and diagnostics;
- closed content-root family rules;
- manifest-to-`ProjectIndex` lowering;
- deletion and migration order;
- a row-by-row test matrix covering the cases above;
- explicit non-goals and compatibility statement; and
- `OPEN_QUESTIONS=0`.
