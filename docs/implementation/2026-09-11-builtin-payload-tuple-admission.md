# Builtin payload Tuple admission — 2026-09-11

Base: `663d60720861b838d864317b43a644442c4b823b`, on the existing `main`
checkout, equal to `origin/main` when inspected. The callable/effect and
nominal schema/producer migrations remain dirty and are preserved.

## Result

Core builtin value constructors put each present payload inside a one-item
Tuple. `RuntimeCheckedType::try_builtin_variant` previously checked only case
count and payload presence, so it could issue a checked type with a scalar,
empty Tuple or multiple-item Tuple that disagreed with the value ABI.

The core case registry now exposes its unary Tuple arity within the crate.
Checked-type construction rejects those malformed payload types with
`RuntimeBuiltinVariantTypeError::InvalidPayloadTuple`, retaining the exact
semantic case coordinate. Both borrowing and consuming builtin value
extractors use the same registry arity. Unit cases retain their absent payload.
The owning builtin identity also exposes its derived payload-item count for
schema producers; no second case table, name or ordinal inventory is stored.

The test exercises every case in all seven current builtin families through
the actual checked-type and value constructors, and rejects each malformed
payload shape for every payload-bearing case. Payload item types remain the
caller's structural input; this constructor does not prove standard-library
semantic signatures, source provenance or generation correlation.

Only these registry/construction/extraction hunks in `pattern.rs` and
`value.rs`, their focused test, and this record belong to the cut. The
mandatory nominal layout, borrowed value visitor, generic builtin schema,
Choice, graph and codec migration remain unstaged. Valid builtin values and
checked-type identity transcripts retain their existing representation and
version 1. No compatibility reader or replacement builtin carrier is added.

## Validation

Commands ran in the existing working tree, including the preserved WIP;
these are not isolated executions of the staged tree. Logs are under
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`.

- Passed: full core all-feature library and integration tests, 476 tests
  (443 library and 33 integration),
  `schema-builtin-complete-core-tests-retry.log`.
- Passed: the constructor regression test after the Clippy style correction,
  `builtin-tuple-focused-tests.log`.
- Passed with warnings: core all-target/all-feature Clippy,
  `schema-builtin-core-clippy.log`. Workspace Clippy subsequently rechecked
  core after correcting the new collapsible-if warning; no warning-free
  result is claimed.
- Passed: changed-core formatting, diff checks and explicit hunk review.
- Passed: `just structure-audit` and `just structure-audit-gate`: 95 packages,
  2,303 Rust files, 1,275,825 physical Rust lines, 311 review triggers and zero
  blockers. The `schema-builtin-structure-*.log` results cover the complete
  dirty checkout.
- Failed: workspace all-target/all-feature check and Clippy,
  `just test-workspace`, and `just test-doc`. Each reaches the unresolved
  production nominal Variant layout initializers in Dialogue
  `character_dialogue/schema.rs:83` and runtime-accelerator `external.rs:543`.
  Workspace tests and doctests do not reach execution. Corresponding logs
  use the `schema-builtin-` prefix.
- Not run: Tier 2, whose workspace runtime prerequisites remain
  uncompilable. The independent core constructor cut changes no MCP,
  capture, resource URI or renderer contract.

The wider schema test development first exposed a stale removed codec tag,
then a wrong private test import and a nested test declaration. Those failed
runs are retained in the same log directory; the final full core run resolves
them. No workspace, generation-admission or producer-closure success is
inferred from the core tests.

## Ownership and remaining work

The current complete `pattern.rs` (2,888 lines / 105,369 bytes) and `value.rs`
(3,690 / 132,225) owners retain their existing type/case and value construction
responsibilities. `pattern.rs` contains 680 embedded test lines. The added
behavior belongs to the existing immutable case registry and its constructors, with no new
state, dependency, facade, transport or persistent authority. The focused
embedded test follows that registry boundary. The large existing owners are
review triggers, not new mixed responsibilities introduced by these hunks.

The generic builtin schema work and complete nominal producer, plan, AWBC and
restore migration remain open under the convergence goal and the existing
[producer closure request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md).
This cut implements the existing unary builtin ABI and introduces no design
deviation or claim that public checked-type aggregates are generation-sealed.
