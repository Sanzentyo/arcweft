# Canonical schema writer — 2026-09-11

Inspected `main` at `d539e93296718754f99076feb1e3760c0733f8ae`, equal to
`origin/main`, with an empty index. The existing 76-path callable migration
remains dirty and is outside this implementation cut. This note records the
shared canonical writer change in `arcweft-core`, not completion of nominal
C1-C6 or the [convergence goal](2026-09-08-convergence-goal-plan.md).

## Implemented boundary

The private `CanonicalWriter` and `CanonicalSink` now own primitive encoding
and bounded byte accounting for both schema and runtime-value transcripts.
The duplicate `CanonicalSchemaBytes` writer and recursive schema encoder are
deleted. Schema traversal owns an explicit work stack, including field and
variant suffixes; one iterator is retained per active aggregate rather than
one pending task per sibling.

`RuntimeTypeSchema::try_layout_hash` streams into BLAKE3 through this same
writer. It no longer allocates an intermediate complete byte vector. The
version-1 domain, tags, shortest-varint counts and names, metadata order, and
collection-overflow error remain unchanged. Existing value bytes still use
their same exhaustive value traversal and the shared primitive writer.

This change supplies the common encoder required by the accepted nominal
graph. It does not add nominal definitions, graph references, graph-aware
value acceptance, plan admission, AWBC nominal rows, or program-bound restore.
The [argument reconciliation](2026-09-11-nominal-graph-argument-reconciliation.md)
and [C1-C6 gap review](2026-09-09-accepted-rust-nominal-gap-review.md) remain
the nominal implementation obligations.

## Validation

Passed:

- `cargo fmt -p arcweft-core`.
- `cargo test -p arcweft-core --lib --all-features`: 378 passed, none failed.
- `cargo test -p arcweft-core --lib --tests --all-features`: 411 passed,
  none failed, across seven test binaries (378/1/3/8/6/2/13).
- `cargo check --workspace --all-targets --all-features`: exit 0, with warnings.
- `cargo clippy --workspace --all-targets --all-features`: exit 0, with
  warnings. The new exhaustive schema traversal receives `too_many_lines`;
  its cohesive byte-order state machine is intentionally kept together.
- `just test-doc`: exit 0; 8 doctests passed, none failed or ignored.
- `git diff --check` on the implementation.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking`:
  exit 0; 95 workspace packages, 2,284 Rust files, 1,269,852 physical Rust
  lines, 311 ownership-review triggers, and zero blocking violations.
- Re-enumeration and SHA-256 comparison of all 71 retained review ZIPs:
  no new, missing, or changed archive.

The schema tests compare exact version-1 byte strings for all scalar tags and
nested field/enum metadata, check every undersized byte limit and the exact
limit against both byte and hash sinks, and encode a 20,000-level schema.
The deep input is dismantled iteratively after encoding so ordinary recursive
`Box` destruction does not replace the traversal under test.

Failed: `just test-workspace`, exit 1 at the compiler library tests. The first
recipe command reported 768 passed and 28 failed before stopping; its compiler
binary reported 58 passed and 28 failed. Twenty-seven failures explicitly
report `semantic effect row is not closed` during semantic analysis. The
remaining `semantic_lease_survives_later_entry_selection_failure` test fails
its expectation of an `Analyzed` compilation lease. These are unresolved
callable/effect migration failures; the schema encoder is not on their failed
assertion path. The rest of that recipe, including its CLI commands, did not
run. This is not a passed workspace gate.

Logs are under the ignored directory
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`, with the
`core-schema-writer-` prefix.

The preserved callable migration has a separately recorded sema result of
757 passed and 79 failed. That result is historical context, not a passed
workspace gate or an accepted regression baseline for this cut.

## Ownership review

`arcweft-core::entry::schema` owns the persistence schema algebra, its value
validation, canonical primitives, and private sink accounting. Its current
`schema.rs` is 56,915 bytes and 1,633 physical lines (1,755 at the inspected
base), including 240 embedded test lines. It remains above the production
ownership-review trigger.

The decomposition follows a real state boundary: `schema/encoding.rs` owns
the schema traversal stack and exhaustive transcript order, with its exact
byte and depth tests in `schema/encoding/tests.rs`. Primitive writer state
remains shared with the value transcript in the parent. No public API is
widened for the split; no second schema or byte authority remains. The parent
tests still exercise its value sinks, ordered values, and primitive encoding.
The new traversal and its tests are 5,848 bytes/142 physical lines and
5,530 bytes/155 physical lines respectively. No manifest, feature, dependency
edge, I/O capability, or cross-crate owner
changes. The remaining parent responsibilities share the persistent-schema
and canonical-value boundary; this cut does not split them merely to reduce
LOC. At its recorded scan, the audit includes the preserved callable WIP and an unlinked
`schema/nominal.rs` draft, which is outside this commit and has no compilation
or acceptance credit from these commands.

Tier 2 is not selected: no render, Agent, MCP, capture, subprocess, or native
attachment behavior changes. No contract version, public data shape, or
accepted design rule changes in this cut.
