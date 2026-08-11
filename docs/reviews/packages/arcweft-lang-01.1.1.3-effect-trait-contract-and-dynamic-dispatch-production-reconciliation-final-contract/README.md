# Lang-01.1.1.3 final contract package

```text
CONTRACT_ID=Lang-01.1.1.3
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
PRODUCTION_CHANGES_IN_THIS_PACKAGE=none
```

This package is the independently usable, design-only production reconciliation
for Lang-01.1.1 rows A023, E014-E017, E022, and E023. It closes the effect
contract of trait requirements and implementations, the shared callable/effect
identity, diagnostics and source ownership, method values, static-witness
dispatch, publication order, and the disposition of the parent contract's E017
dynamic-trait-object row.

## Normative result

- `CheckedCallableCatalog` is the sole checked effect authority.
- `CheckedCallableId` is the sole sema identity used by trait requirements,
  implementation methods, inherent methods, resolver candidates, effect graph
  edges, method values, static witnesses, project indexing, and tooling.
- Compiler lowering projects a checked method ID once into the existing general
  `RuntimeCallableId`; runtime-plan code carries that opaque projection plus a
  typed plan-local `RuntimeTraitMethodId`. Runtime never reconstructs a trait,
  method, implementation, or effect row from names, local indices, or source.
- An omitted effect row on a bodyless authored trait method is represented as
  the actual closed row `{}`. It is not absent, unknown, or inferred.
- The implementation body's final inferred row is checked as a subset of the
  substituted requirement row.
- E015, E016, E022, and E023 use typed diagnostics and revision-bound spans from
  the same diagnostic object for CLI and LSP.
- Parent row E017 is explicitly superseded for Lang-01.1.1. Dynamic trait
  objects remain a future language feature. Replacement row `E017S` covers the
  currently supported static-witness dispatch and is not dynamic-dispatch
  evidence.
- No compatibility alias, shim, dual reader, source gate, spelling-specific
  removed-syntax diagnostic, source reparse, or string-ID fallback is allowed.

## Consumed inputs

| Input | SHA-256 / revision |
|---|---|
| Canonical request Markdown | `65b912d18765c24fcad7f195ef4a6914992fd28b220ec4fc11043e04e9ee7330` |
| Parent contract ZIP | `ed469929680ddeb2c656577d2a049f0d8954b085fd20e2281291630974e01930` |
| Inspected pushed `main` Git commit | `0b7e095f4193b9f7fbbc95cc350a626a8a63640a` |
| `AGENTS.md` blob at that commit | `e91f99213dde67953beda6aa078c370a8dc4541d` |

The parent ZIP passed `unzip -t`; all twelve members listed by its internal
`MANIFEST.sha256` matched. The request, all parent members, the latest pushed
`main`, root `AGENTS.md`, and the complete supplied Rust skill were inspected.
See `REPOSITORY_EVIDENCE.md` for the exact evidence boundary and the truthful
Jujutsu-identity limitation of the GitHub push surface.

## Precedence

This package is a narrow normative correction to the accepted Lang-01.1.1
parent. It supersedes only:

1. the parent's unowned trait-method effect-row boundary;
2. the parent's unqualified dynamic-trait-object statement and E017 row;
3. the parent's unspecified E015/E016 diagnostic contract;
4. the legacy `AWF-EFX-001` upper-bound diagnostic path for E022/E023; and
5. any parent removal evidence that depended on repository source-text scans.

All accepted ordinary-function parsing, direct Await typing and source ranges,
`DirectFrame`/`StreamFactory` classification, direct suspension/cancellation,
AWBC execution, project nominal identity, and Stream wire/opcode work remain
unchanged.

## Contents

- `FINAL_CONTRACT.md` — complete normative contract.
- `IDENTITY_AND_OWNERSHIP.md` — exact typed IDs, owners, fields, constructors,
  visibility, lifecycle, and consumer joins.
- `DIAGNOSTIC_AND_SOURCE_CONTRACT.md` — diagnostic variants, codes, payloads,
  primary and related spans, trace selection, and CLI/LSP projection.
- `DYNAMIC_DISPATCH_DISPOSITION.md` — E017 supersession and replacement E017S.
- `IMPLEMENTATION_ORDER.md` — compile-clean, deletion-driven authority switch.
- `TEST_MATRIX.md` — executable positive, negative, identity, tooling, removal,
  workspace, Clippy, structural, and Tier 2 gates.
- `REQUIREMENTS_TRACEABILITY.md` — request and parent-row closure map.
- `REPOSITORY_EVIDENCE.md` — repository pin, blobs, observed current behavior,
  parent-package verification, and validation scope.
- `SUMMARY.md`, `FINAL_STATUS.md`, and `OPEN_QUESTIONS.md` — package status.
- `MANIFEST.sha256` — filename-sorted SHA-256 and byte length for every other
  member. The manifest does not list itself because a self-hash is recursive.

No Rust source, test, manifest, fixture, schema, stable design chapter, overlay,
or patch is contained in this archive.
