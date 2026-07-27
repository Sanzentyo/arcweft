# Proof convergence: detached fragment payload deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED`

## Context

After Git commit `2b4f0f0107d892d67492d6c3361eb99c590de82a` deleted the
consuming `ParsedSource::into_typed_tree` escape, the public fragment result
still cloned two provisional detached syntax payloads:

- `ParsedFragmentKind::Expression(Box<Expr>)`; and
- `ParsedFragmentKind::Items(Vec<Item>)`.

A workspace consumer audit found no production reader of the expression
payload. Item consumers used only the nonempty result as family evidence; the
sole field-level reader was a test that inspected cloned recovery `Item::Raw`
nodes even though the full-document parser test already proves the same
removed-role rejection behavior. Keeping those values public would preserve
detached syntax ownership without a consumer immediately before the final
Proof attached-fragment authority switch.

`ParsedFragmentKind::Statements(Vec<Stmt>)` is different. The Agent REPL
binding owner reads statement declarations and expression components to build
its binding snapshot. Its final replacement depends on the corrected Proof
`01.1.1.4.1` expression/leaf contract and is therefore deliberately retained.

## Deleted authority

- replace the expression and item variants with unit family markers;
- validate expression syntax without publishing an owned `Expr` clone;
- inspect the item parse product by borrow and stop cloning its `Item` arena;
- classify a clean trivia-only item fragment as complete with no parsed family,
  preserving the existing REPL rule that only a nonempty typed item family is
  an item cell;
- retain invalid-item family evidence only when recovery produced at least one
  item, without exporting the recovery nodes themselves;
- migrate tooling, Agent REPL, and CLI consumers directly to the final unit
  variants; and
- add compile-fail evidence that downstream callers cannot reconstruct the
  deleted expression or item payload constructors.

No compatibility variant, accessor, wrapper, source reparse, source gate, or
removed-syntax diagnostic replaces the deleted payloads.

## Deliberately retained boundary

This cut does not claim that the current fragment parser is the final Proof
owner. It retains:

- `Statements(Vec<Stmt>)`, because two production binding paths consume it;
- `parse_fragment`, completion and error evidence used by interactive tooling;
- the private `parse_source_with_options` item route; and
- the provisional statement/expression parser internals.

Deleting those boundaries requires the corrected Proof `01.1.1.4.1` schema and
one compiling attached-fragment/Agent binding authority switch. The explicitly
`NOT_READY` return is recorded in
[`2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md`](2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md),
so this cut does not infer the missing HIR expression payloads.

## Validation

The implementation is Jujutsu change
`lrnxzmwyzypxyttouzllvsknwpuqzwxv` over parent Git commit
`2b4f0f0107d892d67492d6c3361eb99c590de82a`.

The final checkout passed:

- `cargo fmt --all -- --check` and `git diff --check`;
- all seven fragment parser unit tests;
- the removed-role declaration integration test;
- the syntax public-API trybuild suite, including
  `removed_fragment_payloads.rs`;
- all tooling targets and features;
- all Agent REPL targets and features;
- the nine affected CLI Agent REPL library tests;
- all-target/all-feature check and strict Clippy for syntax, tooling, Agent
  REPL, and CLI;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`; and
- `just test-tier2`, including all 22 MCP stdio tests, Agent observe/capture
  groups, visual smoke, and four checked-in visual goldens.

`just test-workspace` ran for 950.7 seconds. Every workspace, CLI,
integration, and compile-fail component before the Arcweft fixture gate passed,
including the new fragment payload compile-fail row. The fixture gate retained
its exact existing three-pass/two-fail baseline:

```text
tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

Those failures are the already recorded external-capability `FsError`
publication gap and do not call the fragment parser. The only recipe component
after that nonzero exit, the persistent-cache build CLI golden suite, was run
separately and passed both tests.

The canonical structural audit generated reports under
[`structure-audits/proof-fragment-detached-payload-deletion-2026-07-27/`](structure-audits/proof-fragment-detached-payload-deletion-2026-07-27/).
It scanned 3,743 files, including 1,948 Rust files and 906,478 Rust physical
LOC across 95 manifests, and found zero errors and 146 existing warnings. The
warning report is line-for-line identical to the parent audit. Running with
`--fail-on-violations` therefore exits nonzero on the existing warnings; the
canonical dry run exits successfully with the same zero-error result.

Changed production-file measurements are:

| owner | bytes | physical LOC | embedded test LOC | responsibility |
| --- | ---: | ---: | ---: | --- |
| `arcweft-lang-syntax/src/parser/fragment.rs` | 11,724 | 385 | 136 | fragment parsing, completion, and family evidence |
| `arcweft-tooling/src/agent_repl.rs` | 29,002 | 914 | 185 | shared Agent REPL classification and completion |
| `arcweft-agent-repl/src/source.rs` | 8,755 | 240 | 83 | Agent REPL cell classification and source synthesis |
| `arcweft-cli/src/app/agent/native/repl.rs` | 48,877 | 1,477 | 0 | native CLI REPL orchestration and source synthesis |

The CLI file remains above the 1,200-LOC warning threshold, but this cut changes
only two unit-variant matches, adds no responsibility, changes no dependency
edge, and leaves the parent warning inventory unchanged. The three changed Rust
integration/compile-fail files are respectively 15, 62, and 15 physical LOC.

The recursive review-package ledger contains 30 ZIPs; every exact SHA-256 is
recorded in an implementation intake note, the unrecorded count is zero, and
the root review inbox contains no unclassified ZIP.

## Next boundary

After validation and push, select another zero-consumer or source-free reader
whose deletion does not freeze the missing Proof leaf schema. Do not delete the
statement payload or publish a replacement fragment arena until the corrected
`01.1.1.4.1` contract is accepted.
