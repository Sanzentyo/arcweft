# Iterative constraint projection — 2026-09-09

Status: IN_PROGRESS. Supersedes the current validation checkpoint in
[controlled type encoding](2026-09-09-controlled-type-encoding.md), which
remains historical evidence. The [convergence goal](2026-09-08-convergence-goal-plan.md)
and its complete acceptance criteria remain active.

Inspected Git SHA: `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, existing `main`
checkout. HEAD and the local `origin/main` reference have divergence `0/0`;
this checkpoint did not fetch. Inherited changes are preserved. Before adding
this note, the tree had 8 deleted / 648 modified / 108 untracked status entries,
with an empty index. No branch, worktree, reset, commit or push was performed.

## Implemented boundary

The candidate constraint owner now projects transitive type and constant
bindings with explicit traversal frames. The former recursive projector was
removed from `normalization.rs`; the single implementation is in
`normalization/projection.rs`. Active path maps and completed solutions still
provide the same borrowed binding lookup authority.

Type frames retain their child cursor, completed child values, admitted array
length and enclosing lexical scope. A binding frame owns only the cycle guard
introduced while following that binding. Both successful completion and an
error remove those guards without clearing a caller-supplied seed. A scoped
outer call restores the original lexical scope on every `Result` return,
including failure while inside a nested function binder. Constant alias
projection uses an iterative cursor with the same guard ownership.

The existing admission order is preserved: each visited type or constant
charges its node before lookup, array length is projected before the item,
and a function's effect row is validated in its binder before its children.
An array is rebuilt once from its projected length; the old intermediate
reconstruction with the original length has been removed. The closure policies
still distinguish rigid, bindable and future-eligible parameters. This does
not admit missing required parameters or recovery types as successful source
values, and does not change effect substitution rules.

Six new tests extend the existing normalization suite:

- 10,000 type aliases and 10,000 constant aliases close transitively, each
  consuming exactly 10,001 node admissions;
- a 10,000-level vector type reconstructs successfully, while a 32-node
  budget rejects admission 33 before reaching the leaf;
- nested type and constant function binders preserve their original type;
- a nested budget abort restores a nonempty enclosing scope and seeded
  caller guards;
- cyclic type and constant aliases retain the Hint/closed-policy distinction
  and preserve unrelated caller guards; and
- an invalid array length is rejected before an escaped item reference.

The deliberately deep test inputs and outputs are consumed iteratively in the
tests. Ordinary owned `TypeKind` drop and clone are separate operations. This
change does not claim to bound recursive destruction of a completed deep
sibling after a later error, or all surrounding type operations. The existing
occurs/equality routines, runtime-type normalization, surrounding copies and
unmetered encoding entry points remain work for the complete bounded pipeline.

## Remaining connected implementation

The three contextual-constructor and four inferred-callback semantic failures
remain unchanged. Current source inspection confirms that
`AnalyzerExpressionExpectation::Parametric` keeps parent inference references
out of an independently completed child call. A child candidate opens its own
parameter scope; rigid rows admit declaration-Free identities, not another
call's active inference variables. Therefore neither passing a partial type
through the complete-expectation path nor relabeling parent variables as rigid
is a valid repair. Complementary partial evidence must be solved within the
same correlated source-constraint authority before publication. The existing
[coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
retains that requirement; no new source graph or effect algebra is implemented
or declared accepted by this note.

The Character factory execution pair also remains failed. Inspected production
evidence distinguishes the missing boundary from a missing semantic type:

- sema already supplies `CheckedCharacterDialogueFactory` and
  `CheckedCharacterDialogueReconfigure`, each with a typed target and
  source-ordered patch fields;
- runtime type projection already admits the CharacterDialogue opaque carrier;
- executable call projection still rejects the Dialogue family without a
  typed runtime operation; and
- the dialogue owner has the schema, immutable configuration, patch and opaque
  value codec, but these are not connected to ordinary executable value
  construction. Dynamic content target projection separately rejects a target
  without an exact character.

Skipping these calls, constructing only empty constant values, or importing
the dialogue owner into core would not complete that boundary. Source operand
order, immutable reconfiguration, schema ownership and both native/AWBC
execution must be connected. No CharacterDialogue source or test was changed
in this follow-up.

There is no external blocker. Function schemes, callback effects, source
completion, CharacterDialogue execution, Match, View, RuntimePlan/task-plan,
nominal and scheduler/restore remain required. This checkpoint neither narrows
those requirements nor makes the connected working tree ready for a main push.

## Validation actually run

| Command | Result |
| --- | --- |
| `cargo check -p arcweft-lang-sema --all-targets` | Initial extraction failed: an obsolete eighth argument remained at the `seal_type` call, and two imports were unused. These were removed before the successful tests and workspace check below. |
| `cargo test -p arcweft-lang-sema --lib types::constraints::normalization::tests -- --nocapture` | Passed: 11/11, including six new tests. |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Failed: 758 passed / 7 failed / 765 total / 0 ignored. The seven failure names match the preceding checkpoint. |
| `cargo test -p arcweft-compiler --lib --test project_function_instances --test project_cache_transaction --test callable_execution --test evaluated_effects --test try_pipe --no-fail-fast -- --nocapture` | Library passed 82/82. `callable_execution`: 53 passed / 18 failed. Other targets passed 11/11, 19/19, 9/9 and 8/8; integration total 100 passed / 18 failed. The nine paired failure families match the preceding checkpoint. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, exit 0. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-constraint-projection --fail-on-blocking` | Passed: 95 packages, 2,235 Rust files, 309 review triggers, zero blocking violations. |
| `cargo fmt --all`, then `cargo fmt --all -- --check` | Passed. |
| `git diff --check` | Passed. |
| Documentation link check | Passed: 2 documents, 23 relative targets, zero missing files. Anchors were not checked. |

Clippy summaries: sema library/test-library 1,204/1,396 warnings (1,202
duplicates); compiler library/test-library 223/227 (220 duplicates), plus
other workspace warnings. The projection loop has a 127-line Clippy review
warning. It retains one admission/unwind state machine; its type/constant
guards, scope restoration and reconstruction are coupled. No suppression or
additional projector was added to silence that warning.

Logs are `target/constraint-projection-iterative-check.log`,
`target/constraint-projection-focused.log`, and
`target/constraint-projection-{sema,compiler,workspace-check,workspace-clippy,structure-audit}.log`.
All listed Cargo commands ran sequentially with Cargo's normal concurrency.
Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not
run; they remain required at the connected main push cut. There are no newly
blocked validation tiers and no claim of a warning-free result.

## Structure and design disposition

Generated [file measurements](structure-audits/2026-09-09-constraint-projection/file_metrics.csv)
and [dependency measurements](structure-audits/2026-09-09-constraint-projection/package_metrics.csv)
describe the working tree, including inherited changes.

| Sema file | HEAD → current physical LOC | Current bytes | Classification |
| --- | ---: | ---: | --- |
| `types/constraints/context.rs` | 1,282 → 1,695 | 62,467 | production |
| `types/constraints/normalization.rs` | 884 → 613 | 19,960 | production |
| `types/constraints/normalization/projection.rs` | new → 366 | 14,590 | production |
| `types/constraints/normalization/tests.rs` | existing untracked → 524 | 18,194 | test |

All four files have zero embedded test LOC. The context remains the sole owner
of lexical scope and node accounting. Its narrow scope entry/restore methods
support explicit traversal frames without exporting context state to another
crate. Projection was extracted at its real responsibility boundary; policy,
transitive binding lookup, cycle guards and reconstruction remain together.
The parent module retains path normalization, equality and occurrence
relations. The context's size-triggered ownership review is resolved by this
explicit cohesion justification, not a second scope inventory or physical
splitting without an ownership boundary.

Sema workspace fan-in/out is 8/14, with development fan-in/out 3/0. No
dependency, facade, public cross-crate contract, codec version, unsafe code or
I/O boundary changed. No version marker was added or changed; version `1`
remains the required contract. No maintained language rule changed; this is an
implementation and validation checkpoint.
