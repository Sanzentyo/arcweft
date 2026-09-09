# Bounded Pattern completion and recovered Closure projection

Date: 2026-09-10. Inspected base:
`43d4270b4a3b9d813186fa49ed5029591875c416`, `main`, initially clean and equal
to `origin/main`. Implementation and validation below used the dirty working
tree based on that revision.

Supersedes only the recovered-Closure publication finding in the
[Ruby source-projection record](2026-09-09-dialogue-source-projection.md).
Other findings and validation in that note remain historical evidence. The
[whole convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

## Established result

Each Pattern now closes its complete enclosing-grammar region through
`PatternProjectionTransaction::finish_node`. A completed tuple, sequence,
record or variant no longer leaves its suffix outside its own CST node while
claiming that suffix in its typed Whole range. Literal, entity-reference and
discard patterns no longer silently consume and accept unrelated suffixes.

The shared completion owner consumes remaining input, emits one bounded
diagnostic, and records `UnexpectedTrailingInput { token_count }` with an exact
`TrailingInput` source component. It preserves the recognized semantic family,
children, delimiters and binding identities. Generic error-pattern `Recovery`
and completed-pattern `TrailingInput` remain distinct source roles. Variant
payloads retain their own delimiter-bounded child region; the variant root
owns a later suffix. Missing Patterns and method receivers use the same
completion authority without changing their insertion or receiver semantics.

HIR carries the same typed issue and source role. Ordinary Pattern freeze now
re-derives the entire poison state from syntax and the actual Pattern/Type
arenas, replacing the separate container-only recovery filters and the Type
resolver that always returned absence. Candidate Pattern validation already
uses the common state projection and now receives consistent source owners.
No source equality check, poison admission or atomic-publication invariant is
weakened.

The maintained [Pattern chapter](../01-language/patterns-and-bindings.md)
states the complete-region rule. The multiline predicate-header fixture used
`let (left, right): (T, T) = pair`; the old tuple emitter ignored the trailing
annotation and the test only checked CST/header assembly. It now uses the
existing typed-binding grammar, `let (left: T, right: T) = pair`, and verifies
two typed-binding nodes. This correction does not introduce general Pattern
ascription or a spelling-specific compatibility path.

## Reproduction and remaining design boundary

On the base executable, this source failed with
`hir.lower.project_transaction`:

```arcw
entry cli @entry.main { goto @flow.main }
pub character alice { display = "Alice" }
flow main { alice()[|[夢](ゆめ)] }
```

The retained Index interpretation contains a recovered Closure whose Pattern
is `[夢](ゆめ)`. Before this cut, the Pattern's staged range included `(ゆめ)`
but its CST stopped at `[夢]`; `AttachedCandidateNode::closure_parameter`
correctly rejected that mismatch.

The corrected source and candidate projections publish through HIR. The
candidate remains recovered, including its missing Closure terminator/body.
A subsequent compiler probe with the valid Dialogue interpretation still
fails at readiness with `hir.project.execution`: module-level recovery blocks
semantic selection. Runtime Content assertions, AWBC generation and codec
round trip in that probe were therefore **not reached**. The first Ruby
spelling failed before the probe loop reached the second spelling.

The rebuilt CLI subsequently confirmed the same readiness failure for the
original reproduction. A separate control using `alice()[｜夢《ゆめ》]` passed
`arcw check` on that same binary. This is check/verification evidence only;
neither CLI check executes the scene or proves AWBC acceptance.

This is an internal design boundary, not an external blocker and not a reason
to exclude Ruby from the goal. The complete decision is recorded in
[conditional syntax recovery/readiness](../reviews/requests/2026-09-10-conditional-syntax-recovery-readiness.md).
It joins candidate ownership, diagnostics, semantic selection, readiness and
cache admission. This cut does not guess that authority by changing a global
readiness check or discarding losing candidates.

The exploratory compiler test and its complete harness are retained locally
as `compiler-ruby-probe.rs`, with the failed command/output in `compiler-ruby.*`.
It is not installed as a passing regression, ignored, or redefined to expect
the incorrect rejection. The request retains the positive executable/AWBC
acceptance for its owning implementation cut. The committed compiler
regression instead proves that ordinary malformed Closure parameters remain
rejected.

## Validation

Logs and raw artifacts are under the ignored directory
`.arcweft-local/validation/2026-09-10-recovered-closure-projection/`.
No Cargo job-count override, feature/profile workaround or cleanup was used.

Performed focused work:

- The two new initial syntax regressions both failed before production edits:
  silently accepted trailing input and inaccessible recovered Closure view.
  Both passed after shared completion was installed.
- HIR focused regression first passed 3 tests and failed 1 test-harness
  assumption: clean-state substitution was already rejected at the immutable
  slot boundary. The test now verifies that rejection and separately checks
  same-poison count substitution at final source validation.
- `cargo test -p arcweft-lang-syntax -p arcweft-lang-hir --lib --quiet` passed
  all **896 HIR tests**, with **8 existing ignored tests**, then found the
  predicate fixture described above: syntax **672 passed / 1 failed**.
  Total command time was 211.46 s; HIR execution itself was 173.47 s.
- After that test-only correction, `cargo test -p arcweft-lang-syntax --lib
  --quiet` passed **673/673**. HIR production and its tested fixtures did not
  change after the successful full HIR run.
- `cargo test -p arcweft-compiler --test evaluated_effects
  ordinary_closure_parameters_reject_trailing_input -- --exact --nocapture`
  initially passed **1/1**, covering four malformed parameter families. At
  review, that regression moved to its owning `callable_execution` integration
  test and was strengthened to require the original syntax diagnostic and its
  exact `trailing` source span. The final command with `--test
  callable_execution` passed **1/1** in 10.41 s.
- The valid-Ruby compiler probe **failed** as recorded above. It does not
  count as successful compiler, runtime or AWBC acceptance.

Mainline validation:

| Command / tier | Observed result |
| --- | --- |
| `cargo fmt -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-compiler`, then compiler formatting after the final test move | Passed |
| `cargo fmt --all -- --check` | Passed; 10.76 s |
| `cargo check --workspace --all-targets --all-features` | Passed; initial 45.54 s, final 3.96 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; initial 46.22 s, final 3.24 s |
| `just test-workspace` | Initial 856 passed / 18 known failures / 84 reports, 325.64 s; final 857 passed / the same 18 failures / 84 reports, 39.22 s |
| `just test-doc` | 95 reports, 8 passed, no failures; 96.49 s |
| `just test-tier2` | MCP 4 passed; basic native capture 1 passed; first auxiliary test failed on the existing `samples/image-animation.arcw` declaration parse error; 56.95 s |
| Canonical structural audit with `--fail-on-blocking` | 95 packages, 2,250 Rust files, 309 review triggers, 0 blocking violations; 7.11 s |
| Documentation links and `git diff --check` | Passed; 4 documents / 43 local targets, anchors not checked |

The final test-only move/diagnostic strengthening prompted another check,
Clippy and workspace run. Doctests and native Tier 2 were not repeated after
that test-only change. The workspace recipe stops in `callable_execution`
(final: 54 passed / 18 failed), before later workspace and CLI recipes. Those
later recipes are not reported as passing. Remaining auxiliary captures,
visual goldens and production proof-limit Tier 2 targets were not run after
the first auxiliary failure. The known sema 7 failures remain part of the
parent goal; no fresh full-sema result is claimed by this cut.

The review ZIP inventory was re-enumerated and SHA-256/byte lengths recorded:
71 archives, 4,802,433 bytes, no inbox ZIP. No archive was selected, changed,
or newly declared implementation-ready by this source correction.

## Ownership review

No dependency, feature, crate, schema version, I/O owner, unsafe boundary or
facade export is added.
Pattern grammar and source ownership stay in syntax; HIR recovery, arena
validation and source admission stay in HIR. The compiler change is a
rejection regression only.

The table records final dirty-checkout values from the canonical audit;
paths are relative to the named crate. `syntax`, `HIR` and `compiler` denote
`arcweft-lang-syntax`, `arcweft-lang-hir` and `arcweft-compiler`. Every listed
file has zero embedded test LOC; tests live in their own test-classified files.

| Crate | Path | Class | Base → final LOC | Bytes |
| --- | --- | --- | --- | --- |
| syntax | `src/parser/pattern.rs` | Production | 1276 → 1282 | 41877 |
| syntax | `src/parser/pattern_projection.rs` | Production | 763 → 796 | 28047 |
| syntax | `src/patterns.rs` | Production | 683 → 695 | 20581 |
| syntax | `src/patterns/bindings.rs` | Production | 380 → 365 | 12294 |
| syntax | `src/patterns/source.rs` | Production | 1011 → 1028 | 34126 |
| syntax | `src/parser/pattern/tests.rs` | Test | 589 → 692 | 24148 |
| syntax | `src/parser/predicate_proof_tests.rs` | Test | 1680 → 1684 | 57813 |
| syntax | `src/attachment/expression/tests/candidate_control.rs` | Test | 651 → 677 | 26915 |
| HIR | `src/final_lowering/pattern_lowering.rs` | Production | 1300 → 1305 | 49040 |
| HIR | `src/module.rs` | Production | 2058 → 2063 | 83008 |
| HIR | `src/pattern.rs` | Production | 1039 → 1045 | 37405 |
| HIR | `src/source_index.rs` | Production | 1731 → 1733 | 57038 |
| HIR | `src/source_index/pattern_projection.rs` | Production | 1344 → 1352 | 54194 |
| HIR | `src/source_index/pattern_projection/payload_validation.rs` | Production | 565 → 527 | 19601 |
| HIR | `src/source_index/tests.rs` | Test | 3019 → 3031 | 106071 |
| HIR | `src/final_lowering/pattern_lowering/tests/attached_matrix.rs` | Test | 1792 → 1853 | 70741 |
| HIR | `src/final_lowering/pattern_lowering/tests/payload_freeze.rs` | Test | 197 → 263 | 9901 |
| HIR | `src/final_lowering/expression_lowering/tests/dialogue_candidate_control.rs` | Test | 517 → 554 | 21346 |
| HIR | `src/final_lowering/expression_lowering/tests/dialogue_desugaring.rs` | Test | 166 → 167 | 7111 |
| compiler | `tests/callable_execution.rs` | Test | 563 → 588 | 14970 |

Workspace normal fan-in/out: syntax 12/2, HIR 10/3, compiler 3/23.
Development fan-in/out: syntax 1/1, HIR 1/0, compiler 1/5. No edge changes.
Generated reports and the complete measured table remain in the local
validation directory.

Touched size-trigger dispositions:

- Syntax `parser/pattern.rs` owns the complete Pattern grammar, delimiter and
  child emission. Its existing projection transaction now owns completion;
  the dispatcher's family emitters all pass through that boundary. This is a
  shared state/source-ownership boundary, not a Ruby branch or a file split
  for LOC. The existing method-receiver and missing-node paths also use it.
- HIR `final_lowering/pattern_lowering.rs` owns Pattern/Local allocation,
  binding plans and deterministic poison projection for source and candidate
  inputs. The new issue belongs to that same conversion. It adds no semantic
  inference, runtime ownership or alternate child traversal.
- HIR `module.rs` owns atomic whole-module admission. Supplying the actual
  Type arena to Pattern validation belongs to that existing sequence; module
  status, candidate selection, cache eligibility and diagnostics are unchanged.
- HIR `source_index.rs` retains the shared source-query algebra;
  `source_index/pattern_projection.rs` retains Pattern role applicability and
  exact attached manifests. The payload-validation child module now consumes
  real Pattern/Type arenas through the existing resolver contract. The
  container-only filters and separate partial resolver are deleted, so there
  is no second recovery inventory to update for each new grammar issue.
- HIR `source_index/tests.rs` retains shared source/slot/arena fixtures and
  cross-family source-index admission tests. The changed typed-binding
  fixture now returns its real Type snapshot, and the binding-only fixtures
  explicitly supply an empty Type arena. No production API is widened to
  move these fixtures between files. The Pattern attached-matrix and payload
  freeze tests stay under the owning Pattern lowering test module.
- Syntax predicate/proof tests retain header, logical-line and declaration
  assembly responsibility. Only the unsupported ignored suffix in one fixture
  is replaced with two real typed bindings; malformed Pattern coverage belongs
  to the separate Pattern test module.
- The compiler rejection test belongs to the callable integration suite and
  validates typed diagnostic/source behavior at the real compiler entry point;
  it does not remain in the unrelated Content/Fx observation fixture.

These are cohesion decisions based on state, dependency, API and test
ownership. Numeric size alone is not reported as a structural pass or failure.

## Remaining acceptance

Conditional candidate recovery/readiness, image-animation sample parsing,
the known sema 7/callable 18 failures, and the full Match/View/task-plan/
nominal/scheduler sequence remain required. This is a coherent Pattern
ownership correction, not completion of those connected tasks. The separate
design request is preparation for implementation, not an exemption or an
external wait condition.
