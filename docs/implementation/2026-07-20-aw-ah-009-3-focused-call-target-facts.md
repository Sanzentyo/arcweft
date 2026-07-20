# AW-AH-009.3 focused call-target facts and resolver control

Date: 2026-07-20

## Scope

This cut implements the `arcweft-lang-sema` portion of the native semantic
signature-query path on top of the accepted callable catalog and shared
resolver. It does not publish an LSP result and does not claim completion of
AW-AH-009.3.

The cut is integrated above main `5d33b2ac3e38` after the CharacterDialogue
domain switch and the proof-only trusted-metadata cut. The old dialogue `.say`
surface and
`DialogueCallableId::SpeakerPreset` are not restored as signature-query
families.

## Implemented boundary

- A focused fact request is keyed by the exact accepted `SourceSpan` of one
  parenthesized call. Expression ordinals are retained as checked HIR
  coordinates but are not request identity.
- The checked call target records full immutable `ResolvedCallable` products
  for a selected or ambiguous call. The fact therefore retains the selected
  schema, origin, candidate identity, and considered candidate set without
  reconstructing them from source or reducing them to strings.
- Authored arguments retain exact source, authored name, spread status,
  inferred type, expected parameter type, checked parameter coordinate, and
  monotonic `Clean` / `Recovered` / `Rejected` poison.
- The existing registered-call resolver and existing argument mapper produce
  the facts. No second signature-help resolver or parallel argument
  interpretation was added.
- Focused analysis borrows the caller's exact cancellation flag and mutable
  `ResolverWork`. Resolver cancellation and work exhaustion remain typed,
  terminal results.
- A focused source identity that is absent from the accepted project is
  rejected before style checks, expression checks, or resolver work. The
  foreign-source regression test verifies that the caller-owned work counter
  remains at zero.
- Focused facts live in a crate-private focused result wrapper. The public
  `TypeCheckReport` remains structurally unchanged, so downstream verifier
  tests and consumers that construct the established report shape are not
  broken by an internal query carrier.
- Ordinary type checking uses one immutable never-cancelled control and does
  not construct a fresh `AtomicBool(false)` for each call. Its one owned work
  counter is reused and reset to the production per-call limit before each
  ordinary resolution, preserving the previous resource boundary without
  sharing a cumulative module-wide budget.
- Disabled fact recording reports that no call is wanted and retains no fact,
  error, or argument-fact allocation.

## Preserved argument policy

The fact path consumes the same published argument schema as ordinary
registered-call checking:

- positional and named parameters preserve their checked mapping;
- unknown named arguments follow `Reject`, `OpenChecked`, or `OpenUnchecked`;
- unchecked and rejected spreads remain one authored, unmapped slot;
- fixed-literal spreads expand only when the schema policy authorizes that
  expansion;
- typed-rest spreads retain their existing item checking; and
- a rejected value or type mismatch does not incorrectly masquerade as a
  spread-shape failure when deciding whether to report missing parameters.

## Current family coverage

This cut records facts for the registered catalog paths already shared by
ordinary checking:

- registered project and environment free calls; and
- registered environment method calls.

It deliberately does not approximate the remaining checker-owned families.
FX, enum constructors, builtins, Agent intrinsics, presentation calls,
inherent methods, trait methods, data-last fallback, and function values still
need one native query/result-builder integration or explicit
non-applicability evidence. Dialogue query coverage must follow the accepted
CharacterDialogue surface; it must not revive `.say`, speaker presets, or an
obsolete content-call inventory.

## Remaining AW-AH-009.3 work

1. Build the position-aware native semantic query from the accepted
   document/HIR lease and the focused sema facts.
2. Reconcile every still-valid callable family with the shared typed result
   builder.
3. Make focused traversal/selectivity and request work charging explicit. The
   current focused checker shares the caller budget correctly, but a target
   late in module traversal can consume work after earlier registered calls.
4. Implement the LSP Cut 6 cache/lease bridge and delete the legacy word-based
   signature-help fallback.
5. Run the broad integration and Tier 2 route when the LSP/public request cut
   crosses the runtime, Agent, or MCP boundary.

## Validation

Completed for this sema cut:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema callable::resolver_tests --lib -- --nocapture
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema checker::call_target_facts --lib -- --nocapture
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --lib
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
rustfmt --edition 2024 --check <changed arcweft-lang-sema Rust paths>
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check -- <changed arcweft-lang-sema paths>
```

Results:

- callable resolver tests: 20 passed;
- focused recorder/control tests: 2 passed;
- complete sema library suite: 780 passed;
- warning-denying all-target/all-feature Clippy: passed;
- changed Rust paths pass `rustfmt --check`;
- canonical structural audit: 3,348 files, 1,721 Rust files, 794,222 physical
  Rust LOC, 92 package manifests, 0 errors and 129 existing warnings; and
- changed-path whitespace audit: passed.

`registered_call.rs` is a production warning-level hotspot at 1,211 physical
LOC. Fact-only construction is already split into
`registered_call/facts.rs`; the remaining file owns the cohesive resolver,
published argument-policy application, and ordinary diagnostics. The
structural audit reports no error-level ownership violation. Enclosing
workspace and Tier 2 validation remains with the parent cut because the shared
checkout also contains the independent Lang-01.5.1 migration.
