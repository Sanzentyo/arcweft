# Proof final-HIR source-role authority decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores source-role schema and traceability only. Historical
validation and progress claims are not current evidence.

## Sole source-role owner

`HirSourceIndex` is the sole authored source-role authority. Its immutable
snapshot retains the typed applicability/requiredness manifest and the present
non-`Whole` components. `Whole` remains slot metadata. Direct lowering derives
the manifest from the exact attached `ParsedSource`; module freeze validates it
against the same syntax identity, final payload family, ordinals, and
transaction lease without reading source text.

The public query order is fixed:

1. resolve the qualified owner and payload in its arena;
2. validate role and ordinal against the frozen manifest;
3. validate logical document, source revision, and retained byte length; and
4. return the committed site or typed optional absence with payload-derived
   poison status.

Expression entity references use
`EntityReference(HirIdRefSourcePart)`, preserving structured ID-reference
coordinates rather than pretending they are language path segments. Expression
roles do not include `TypeRegion`: type regions belong to reference types,
while runtime registry access uses `RegistryScope` and
`RegistryKeySegment`. Statement roles have no ordinal-bearing family.

`HirStmtSourceRole::UnsafeAuditInsertion` is the only statement-owned edit
component. It is a checked zero-width insertion inside a complete attached
unsafe block. Missing or recovered delimiters keep the typed statement family
poisoned and publish no edit. Items and declaration members do not introduce
parallel component-query families; their whole sites remain slot-owned and
their payloads are re-derived from attached items at freeze.

## Expression-role consistency

- A compact numeric sequence validates its recovery ordinal against its
  retained element count; no sparse `max(ordinal, len)` shortcut exists.
- A Dialogue configuration argument keeps its authored sparse call-argument
  ordinal. Applicability is checked against the target call's argument slice,
  never `coordinates.len()`.
- Every RichText end tag owns its Dialogue-node whole component. An end-tag
  tag component exists only for an exact paired start-tag ordinal; names and
  source strings are not used to manufacture pairing.
- `PostfixBracket` is bracket-only. Colon syntax lowers directly to
  `DialogueContentApplication` and cannot pass through an intermediate postfix
  carrier.

The manifest is required because semantic payloads intentionally discard some
authored spelling, including literal components, ID suffix ordinals, invalid
field shorthand/rest spelling, Dialogue bracket-versus-colon form, RichText
argument/end-tag coordinates, type delimiters, and trailing separators. Those
coordinates stay in the typed attached-source projection rather than being
copied into semantic payloads or reconstructed later.

## Deletion and acceptance boundary

The public switch deletes expression-only or item/member source readers,
parallel maps, range reconstruction, detached syntax, and string fallbacks in
the same compiling migration. Current acceptance must exercise each family's
required, optional, inapplicable, poisoned, stale/foreign, ordinal, and
rollback outcomes through the full matrix; this note awards no PASS rows.
