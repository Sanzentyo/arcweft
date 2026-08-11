# Lang-01.1.1.2 implementation-ready final contract

**Contract:** Project-aware nominal type resolution production reconciliation  
**Contract date:** 2026-07-21  
**Audited repository:** `Sanzentyo/arcweft`  
**Audited `main`:** `23ed5d93824630d8ead9092d32f7fc70f0a8f314`  
**Request baseline:** `c56c82240dacc0d254c7d32e17359d4be0f04b41`  
**Status:** `READY`  
**Open questions:** `0`  
**Production implementation performed:** `false`

This archive is the final, independently throwable implementation contract for
Lang-01.1.1.2. It freezes one project-aware nominal type-resolution authority
without implementing production changes.

## Normative result

The existing `arcweft_lang_hir::symbol::ProjectSymbolTable` remains the only
project declaration/import/re-export authority. It is extended in its owning
module with source-backed struct, enum, and type-alias declarations. One
`arcweft_lang_sema::nominal::resolve_type_ref` operation combines that immutable
project selection with generic, built-in, `Self`, projection, accepted
environment, character, adapter, and explicitly open-name evidence.

No second project symbol table, import resolver, alias resolver, checker-only
catalog, entry-only successful lookup, or LSP-only resolver is permitted.

## Preserved substrate

The implementation must preserve, unless a new concrete defect is proven:

- typed prefix/postfix Try and typed Await source evidence;
- `CheckedReturnTarget::{Known, InferredClosure, Unresolved}`;
- nearest-boundary selection and operand-success recovery;
- alias-normalized anonymous-choice checking;
- ordinary generic substitution;
- the AW-AH-009.3 callable catalog/resolver and callable identities; and
- the current unified project-symbol world/revision transaction.

This contract changes only the missing nominal authority and the successful
paths that currently disagree with it.

## How to use this archive

1. Read `FINAL_CONTRACT.md` as the normative design.
2. Apply `IMPLEMENTATION_ORDER.md` in order; each cut is compile-clean and
   removes replaced success paths rather than retaining compatibility readers.
3. Use `OWNER_INVENTORY.md` to migrate or delete every current owner.
4. Use `RESOLUTION_AND_POISON_TABLE.md` while implementing resolver branches.
5. Implement every row in `TEST_MATRIX.csv`.
6. Use `REQUIREMENTS_TRACEABILITY.md` to prove every request decision and test
   family is covered.
7. Run the implementation-phase commands in `VERIFICATION_MANIFEST.md`.

## Artifact integrity

`MANIFEST.txt` records SHA-256 and byte size for every other file in this
archive. It deliberately excludes itself so that the manifest has no recursive
hash dependency. The ZIP is emitted with deterministic entry ordering and
fixed DOS timestamps.

## Verification boundary

The latest accessible `main` was inspected through the GitHub connector and
rechecked immediately before artifact generation. The request, root
`AGENTS.md`, and Rust skill were read in full. Repository files and current
successful resolution paths were audited as listed in
`REPOSITORY_EVIDENCE.md`.

No production source was edited, no repository branch was created, and no
Cargo command was represented as having run. The implementation-phase Cargo,
Tier 2, metadata, diff, and structural checks are mandatory acceptance work and
are enumerated precisely in `VERIFICATION_MANIFEST.md`.

The predecessor ZIP named by the dispatch text was not supplied as bytes in
this conversation. Its stated SHA-256 is retained as dispatch metadata only;
this contract makes no claim about uninspected predecessor contents.
