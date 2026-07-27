# Proof 01.1.1.4.1 NOT_READY redelivery intake

Date: 2026-07-27

Status: `HISTORICAL_SUPERSEDED_TRANSPORT`

This note records the first 1,305-byte `NOT_READY` return. The canonical path
below was replaced on 2026-07-27 by the later 64,523-byte redelivery after its
transport integrity was verified. The original bytes and hash remain in Git
history; they are not an active design package or compatibility input. See
[`2026-07-27-proof-01-1-1-4-1-ready-claim-redelivery-intake.md`](2026-07-27-proof-01-1-1-4-1-ready-claim-redelivery-intake.md)
for the current archive adjudication.

## Archive

- Repository path:
  `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`
- ZIP bytes: `1305`
- ZIP SHA-256:
  `9ccb9af261a3d55bddefe570b4902d9ba6395725904f88bf389b4565e5bd8374`
- Declared and actual `MANIFEST.json` SHA-256:
  `6af812a062aefe5d08b9b9be2ad05759c77893c826023bc9d4ffd394e5d550c9`
- Sidecars: all six files are inside the ZIP; no external sidecar is required.

The archive was moved intact from the `docs/reviews/` intake root into its
sequence design-package directory after verification. The path now holds the
later redelivery; this section describes the historical bytes only.

## Integrity result

The ZIP opens successfully. Every manifest member has the declared byte length
and SHA-256:

| member | bytes | hash result |
| --- | ---: | --- |
| `FINAL_STATUS.md` | 10 | exact |
| `OPEN_QUESTIONS.md` | 105 | exact |
| `README.md` | 94 | exact |
| `VALIDATION_SCOPE.md` | 57 | exact |

`MANIFEST.sha256` exactly matches the included `MANIFEST.json`. The archive is
therefore a valid transport artifact; no corruption or sidecar mismatch is
present.

## Contract result

The package is not an implementation contract:

- `FINAL_STATUS.md` is exactly `NOT_READY`;
- `README.md` says repository/package construction failed;
- `OPEN_QUESTIONS.md` says no implementation-ready contract could be verified;
- `VALIDATION_SCOPE.md` says no repository-aware validation succeeded; and
- the ZIP contains none of the required primary request copy, correction
  request copy, Rust-facing schemas, precedence table, exhaustive lowering
  matrix, Dialogue/RichText contract, deletion order, test matrix, traceability,
  or repository evidence.

This does not supersede the rejected Proof `01.1.1.4` package and does not
authorize any final leaf/expression schema. The existing
[`01.1.1.4.1` request](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
remains the independently throwable request and must be sent again.

## Implementation effect

- final semantic leaf/expression payload: `DESIGN_BLOCKED`;
- final `TypedSyntaxTree` / document-bound lowerer replacement: `DESIGN_BLOCKED`;
- arena HIR/project/compiler/LSP public authority switch: `DESIGN_BLOCKED`;
- leaf-independent deletion of source-free public facades: may continue; and
- no compatibility alias, dual reader, guessed schema, source gate, or
  removed-syntax diagnostic may be introduced while waiting.

The current source-free HIR lowering deletion does not depend on the missing
schema and remains valid. The next leaf-independent cut is deletion of the
public raw-text `parse_source` test facade.
