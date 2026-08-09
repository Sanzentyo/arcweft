# AW-AH-009.3 final public-switch closure

- Date: 2026-08-09
- Git HEAD inspected: `80331c81e338d20e968a10947d5e848c39610384`
- Working copy: dirty public-switch integration on
  `codex/proof-public-switch`
- Status: `COMPLETE_FOR_RETURNED_CONTRACT`

## Final authority

Character nominal signature help now has one production path:

1. the accepted document/HIR/profile generation supplies the exact authored
   call and parser-owned argument range;
2. final sema uses the shared callable catalog and resolver to retain the
   complete considered-candidate set, selected overload, active signature,
   typed Character owner, diagnostics, resource failures, and work accounting;
3. the native query result is projected to LSP without re-resolving source
   spelling; and
4. the bounded native cache keys and revalidates the complete accepted stamp
   before both cache-hit return and post-compute publication.

The old method dispatcher and the former standalone signature-fact integration
test facades are deleted. Their behavior is owned by final sema library tests
and the LSP cache/state/request modules; no compatibility target, fallback
resolver, source reconstruction, or duplicate accounting carrier is restored.

## Current-copy acceptance evidence

The following commands used the ordinary shared incremental target and
`CARGO_BUILD_JOBS=8`:

- `cargo test -p arcweft-lang-sema --lib` passed 163/163, including Character
  nominal `show` parity, overload selection, resource precedence, candidate
  limits, publication rollback, and physical/logical work accounting;
- `cargo test -p arcweft-project-loader --lib --all-features
  generated_metadata` passed 6/6;
- LSP profile cache owners passed 7/7;
- LSP profile state owners passed 12/12;
- LSP signature-cache owners passed 42/42;
- LSP signature/request owners passed 50/50;
- LSP position owners passed 8/8; and
- the direct registered-adapter signature-help owner passed 1/1.

Workspace `cargo check --workspace --all-targets --all-features`, workspace
Clippy for all targets and features with `-D warnings`, and both
structure-audit gates also passed on this working copy.

## Boundary with the next slice

The complete LSP library/integration run has eight Character-definition tests
that reach a valid typed `show(...)` call and then fail closed at compiler
runtime-semantic projection because the typed Presentation command ABI is not
yet available. They do not invalidate signature-help selection or cache
closure. The ABI is owned by the unreturned
[`AW-AH-011/013` request](../reviews/requests/2026-07-14-aw-ah-011-and-013-typed-presentation-command-abi.md),
not by CharacterDialogue. It must not be guessed by deleting `show`,
downgrading the diagnostic, rereading source, restoring the string driver
grammar, or introducing a Presentation shim.
