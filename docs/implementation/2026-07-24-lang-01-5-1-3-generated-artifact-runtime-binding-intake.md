# Lang-01.5.1.3 generated artifact runtime-binding intake

> Superseded on 2026-08-08 by the retained
> [`generated-artifact runtime-binding fail-closed final contract`](../reviews/packages/zips/arcweft-lang-01.5.1.3-generated-artifact-runtime-binding-fail-closed-final-contract.zip)
> at SHA-256
> `342D38E521C14F2CCE340355F4F4BC07241C8BFA89DA9B7C324B169869482027`.
> In particular, the final crate owner is `arcweft-runtime-binding`, not
> `arcweft-artifact-binding`, and the ID is
> `GeneratedArtifactBindingId`, not the older slot model. Retain this note as
> historical audit evidence only; use the
> [2026-08-08 correction intake](2026-08-08-lang-01-5-1-correction-returns-intake.md)
> for implementation authority.

## Status

`ACCEPTED_IMPLEMENTATION_READY_DEFERRED`

The returned design contract is mechanically valid and closes the previously
unreturned Lang-01.5.1.3 boundary. It is not part of the current AW-AH-009.3
production cut. Implementation remains ordered after the prerequisite
single-manifest, typed-resource, topology, and accepted generated-callable
publication slices.

## Returned package evidence

- verified external archive:
  `D:\sanze\Downloads\Lang-01.5.1.3-generated-artifact-runtime-binding-final-contract-eb1a4ea1.zip`;
- archive SHA-256:
  `575DD96CA7ED6131C6B078266E5DDF37DC647026FB04C7F4B418E2C411B416D4`;
- embedded request SHA-256:
  `bfbce1dd3f1e893382930baa6f4e52bc53b5fb78b777c01793961abacaedcec1`;
- repository request SHA-256: the same value;
- package baseline:
  `eb1a4ea1e9f540b6e8ec66137c4e6b15e074e6d3`;
- status: `FINAL`, `FALLBACK_USED=NO`, `PRODUCTION_CODE_CHANGED=NO`,
  and `OPEN_QUESTIONS=0`;
- `MANIFEST.json`: all eleven listed payload hashes and lengths matched;
- `SHA256SUMS`: all twelve listed members matched;
- ZIP entry read/CRC path: all thirteen archive entries were readable.

The external archive was not copied into the repository in this cut. Its
verified path and digest are retained here as allowed by the package-intake
workflow; the binary is not exposed as an active workspace overlay.

## Baseline reconciliation

The package baseline is an ancestor of current `main` at
`5f33ea20fcde7317332c95324701ed4ea7ab813a`. The intervening production
changes add Proof assertion-identity primitives and an LSP fixture split; they
do not replace the generated-artifact topology, callable-provenance,
runtime-target, host-catalog, or binding-plan owners selected by this package.
No correction request is required from that baseline movement.

Implementation must nevertheless re-audit the exact then-current owners at
its coherent cut, rather than treating this intake as evidence that future
production code already satisfies the contract.

## Accepted final boundary

The package selects one typed, fail-closed path:

1. `arcweft-id` owns a checked `GeneratedArtifactBindingSlot`.
2. a new Sans-I/O `arcweft-artifact-binding` crate owns the complete typed key,
   deterministic dense-slot plan, strict schema-1 codec, mismatch taxonomy,
   and Activity routes;
3. accepted topology builds one plan from retained metadata only after the
   authoritative `SourceSetRevision` is frozen;
4. the exact slot crosses adapter publication, accepted nominal projection,
   callable catalog/equivalent-source provenance, selected semantic evidence,
   compiler evidence, and runtime-plan lowering without reconstruction from a
   spelling or path;
5. core carries only the typed slot in original runtime target enums and
   remains Sans I/O;
6. runtime-host owns the immutable exact-key catalog, preflight, function
   binding, and Activity binding; and
7. missing, stale, unselected, mismatched, and duplicate bindings fail before
   argument evaluation, host enqueue, Activity start, scheduler mutation, or
   registry mutation.

The negative mismatch, stale-revision, no-fallback, and no-partial-work matrix
must land before the first successful in-memory binding fixture. Writer and
reader switch atomically; no compatibility reader or slotless generated path
may remain.

## Explicit non-goals

This contract does not authorize dynamic-library loading, WASM
instantiation, process spawning, provider discovery, filesystem probing,
Cargo/WIT execution, artifact download, name/path/profile fallback, a
last-known-good catalog, a dual revision authority, or broad generated
function-value/partial/curried execution.

Lang-01.5.1.3 must not be dispatched again. Later work consumes this returned
contract after its prerequisite slices are production authority.
