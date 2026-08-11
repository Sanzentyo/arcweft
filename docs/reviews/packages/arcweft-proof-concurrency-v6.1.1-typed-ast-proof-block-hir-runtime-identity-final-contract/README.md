# Arcweft proof-concurrency v6.1.1 final production contract

## Status

`READY_FOR_IMPLEMENTATION`

This archive is the decision-complete, implementation-ready production contract for the remaining portions of proof-concurrency cut 01.1. It supersedes every earlier 01.1 design package for the boundaries covered here. An implementation agent needs only this archive, the latest `main` checkout, the current repository `AGENTS.md`, and the applicable Rust skill.

The contract is intentionally design-only. It contains no production checkout, patch, generated build output, cache, credential, or validation log. Production code was not modified while preparing it.

## Repository basis

- inspected repository: `Sanzentyo/arcweft`
- inspected latest `main`: Git `76d39983ad8770a87d6e81745785b6b362a381b4`
- latest repository-recorded validated production substrate: Git `5a36cd0af83085179c299ef50ec8aa786ed731aa`
- repository-recorded Jujutsu identity for that production substrate: `nowqxzku`
- current policy: `AGENTS.md` at Git `76d39983ad8770a87d6e81745785b6b362a381b4`
- inspection method: read-only GitHub connector plus the supplied request and Rust skill

The GitHub connector does not expose the repository's local `.jj` operation store or a longer printable form than the repository-recorded `nowqxzku`. This archive therefore records exactly that authoritative repository value and does not fabricate a longer identifier. This evidence limitation changes no API, grammar, migration, or verification decision.

## Normative document order

When two passages appear to overlap, apply them in this order:

1. `FINAL_STATUS.md` and `OPEN_QUESTIONS.md` establish readiness.
2. `API_AND_DIAGNOSTICS.md` freezes public and crate-owned signatures, errors, diagnostics, constructors, and ranges.
3. Boundary-specific documents freeze behavior and ownership.
4. `IMPLEMENTATION_PLAN.md` freezes compiling order and forbidden intermediate states.
5. `TEST_MATRIX.md` and `VERIFICATION_PLAN.md` freeze completion evidence.
6. `REPOSITORY_EVIDENCE.md` records the inspected starting point and is descriptive, not a competing design authority.

Latest production `main` and current `AGENTS.md` still win when implementation begins. A changed production fact must be reconciled by preserving the decisions and ownership in this archive rather than restoring an obsolete representation.

## Archive map

- `DESIGN.md`: architecture, invariants, scope, and end state.
- `LOSSLESS_TYPED_IDENTITY.md`: grammar-level Rowan tree, identity allocation, reconciliation, typed attachment, fragments, and atomic parse flow.
- `PREDICATE_PROOF_GRAMMAR.md`: final surface grammar, limits, name/import/recursion policy, and recovery.
- `PROOF_BLOCK.md`: exact typed predicate/proof body and block contract.
- `HIR_DATABASE_AND_ARENAS.md`: database, module snapshots, arenas, IDs, liveness, synthetic nodes, resolvers, limits, and lowering transaction.
- `SCOPES_LOCALS_CAPTURES.md`: lexical scope, local, pattern binding, shadow, closure capture, and direct lowering rules.
- `PROJECT_AND_SYMBOLS.md`: module-preserving project view, unified symbol registration, imports, and proof artifact identity.
- `RUNTIME_ASSERTION_FAULT.md`: exact runtime assertion-fault identity, execution inventory, persisted/session boundary, and presentation projection.
- `API_AND_DIAGNOSTICS.md`: consolidated Rust-facing contract and diagnostic table.
- `MIGRATION_AND_DELETION.md`: production file/symbol/caller inventory and required deletion.
- `IMPLEMENTATION_PLAN.md`: safe compiling sequence and cut gates.
- `STRUCTURE_PLAN.md`: measured current hotspots, final responsibility modules, dependency pressure, and post-split size caps.
- `TEST_MATRIX.md`: direct behavioral, atomicity, codec, dependency, and compile-fail tests.
- `VERIFICATION_PLAN.md`: exact commands and honest validation boundary.
- `REPOSITORY_EVIDENCE.md`: confirmed production evidence.
- `REQUIREMENTS_TRACEABILITY.md`: request-to-decision and request-to-test mapping.

## Integrity rule

`MANIFEST.txt` lists all twenty archive members in lexical order as:

```text
<lowercase SHA-256><two spaces><member name>
```

The manifest's own entry uses sixty-four ASCII zeroes instead of attempting an impossible recursive digest. Every other digest is the SHA-256 of the exact uncompressed member bytes. The ZIP is deterministic: lexical member order, DOS timestamp `1980-01-01 00:00:00`, UTF-8 names, DEFLATE compression, and no extra members.

## Completion meaning

`READY_FOR_IMPLEMENTATION` means every result-changing design decision requested for cut 01.1.1 is closed. It does not claim that the described production implementation already exists or that implementation validation commands were run. The implementation is complete only after the code, deletion, tests, workspace checks, Clippy, formatting, dependency evidence, and structural audit in this archive pass on the implementation checkout.
