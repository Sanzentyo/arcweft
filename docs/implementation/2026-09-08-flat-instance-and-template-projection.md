# Flat instance environments and scoped template projection — 2026-09-08

Supersedes the current substitution/instance status in
[source completion](2026-09-08-source-completion-scope.md). Its earlier
validation remains historical. This is an uncommitted migration record, not
completion of the [convergence goal](2026-09-08-convergence-goal-plan.md).

Inspected `D:/git/arcweft`, existing `main`, with HEAD and `origin/main` both
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`. The index is empty. The initial
state for this continuation was 8 deleted / 641 modified / 64 untracked
status entries. Existing changes were retained; no branch, worktree, reset,
commit, or push was performed. Status entries can represent whole untracked
directories and are not file counts.
After the recorded changes and retained audit, status was 8 deleted / 641
modified / 66 untracked entries, still with an empty index and unchanged
HEAD/origin.

## Working-copy result

`TypeConstraintSolution::apply` and the unscoped
`FrozenCallTypeSolution::instantiate_type` were deleted. Completed template
application now returns an owned `ScopedType`; keyed projections retain the
same owned term/context carrier. Its borrowed view has two explicit exits:

- A value must validate at the root. It may lose unused incoming scope only
  after proving every retained bound reference belongs to a binder in the
  value itself.
- A continuation transfers incoming quantifiers into its root function
  binder. Existing root slots follow the transferred slots in each namespace;
  nested binders retain their own coordinates. Type and const references are
  shifted when a replacement is inserted below a template's local binder.

Template type and const substitution is simultaneous. A replacement is not
looked up again in the callee's map. For a recursive application with
`callee T = Array<caller T, caller N>` and `callee N = 7`, the replacement for
T retains caller N; a separate formal N occurrence becomes 7. This prevents
the old type-then-const pass from rewriting the caller's constant.

`FrozenCallTypeSolution` distinguishes declaration value templates,
continuation result templates, and completed source values. The source-value
path projects effects and verifies root scope without reinterpreting its free
type/const declarations as callee slots. Prepared/checked results, checked
argument expectations, receiver/semantic operands, attached content, and
selected callable types were moved to the appropriate role. Analyzer result
publication uses the scoped solver result and transfers its quantifiers.

`ClosedTypeInstantiation` owns a flat environment built from completed
binding values projected once through the enclosing caller. It retains no
enclosing solution or application history. Its template projection requires
all free declarations to have closed bindings while preserving function-local
quantification. `CheckedProjectFunctionInstanceSolution` retains this
environment and the callable ABI closed in the caller's environment. Compiler
instance selection no longer copies an ABI which would subsequently be
reapplied as a callee template.

All old instance layers and `instantiate_*_through` helpers were deleted.
The unused whole-type const substitution visitor and partial layer effect
helper were also deleted. Template application, binder transfer, and closed
instance projection use one structural walker over the existing shared type
shape, including payload owner-only children. Array-length projection has a
direct typed API. Instantiation and callable-join transcripts hash the
appropriate scoped rows directly and propagate scope failures. Contract
domains remain version 1.

This does not complete the effect-reference migration: the existing issuer
overlay still supplies effect rows. Binder effect arities are retained, but
`GenericEffectReference` is not yet the executable row-tail authority.

## Tests and validation

Six new lower tests cover recursive flat composition and absence of caller
history, simultaneous type/const substitution, preservation of local
function binders, incoming/root/nested binder transfer, lifting residual
replacement values, and quantifier arity overflow. They are **NOT RUN**.
Stale lower tests were migrated to active inference keys and scoped completed
views; residual expectations now assert bound slots instead of declaration
Free references. Two existing callable-join tests handle fallible digests.

Actually performed:

- `cargo check -p arcweft-lang-sema -p arcweft-compiler --tests
  --message-format=short`: **FAILED**, exit 101. Latest diagnostic is
  `callable/schema.rs:2489`, missing `binder` in `for_function_value`, reported
  once for the sema library and once for its test target. Log:
  `target/generic-flat-instantiation-migration.log`. Compiler checking and
  all test execution are blocked by this dependency failure. Earlier checks
  in this continuation failed at 5/28 and 1/22 diagnostics; those counts are
  not test results.
- `cargo fmt -p arcweft-lang-sema -p arcweft-compiler`: **PASSED**.
- `cargo fmt --all -- --check` and `git diff --check`: **PASSED**.
  Local Markdown target inspection of this record, the preceding source
  record, and the correction request found 23 targets and 0 missing files.
  Anchor fragments were not validated.
- Canonical structure audit with `--fail-on-blocking`: **PASSED**, exit 0;
  95 packages, 2,218 Rust files, 311 review triggers, 0 blocking violations.
  Command: `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-flat-instance-and-template
  --fail-on-blocking`. Log: `target/flat-instance-and-template-audit.log`.
  Retained generated [measurements](structure-audits/2026-09-08-flat-instance-and-template/file_metrics.csv),
  [package metrics](structure-audits/2026-09-08-flat-instance-and-template/package_metrics.csv),
  [dependency edges](structure-audits/2026-09-08-flat-instance-and-template/dependency_edges.csv),
  and [findings](structure-audits/2026-09-08-flat-instance-and-template/findings.md).
- Native/AWBC execution, workspace all-target/all-feature checks, Clippy,
  workspace tests, doctests, codec/golden, and Tier 2: **NOT RUN**. They remain
  required for the connected implementation cut.

## Structure review

The base below is the complete accepted HEAD file, so growth includes
inherited uncommitted work. Values are physical LOC / bytes / embedded-test
LOC from the current audit. Paths are relative to `crates/`.

| Owner | Base LOC | Current LOC | Bytes | Embedded tests | Disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| arcweft-lang-sema/src/types.rs | 1,330 | 1,712 | 56,186 | 108 | Existing semantic algebra facade; adds the narrow projection error and crate-only scoped carrier |
| arcweft-lang-sema/src/types/generics.rs | 0 | 489 | 14,621 | 0 | Owns lexical scope, contextual views, and checked binder concatenation |
| arcweft-lang-sema/src/types/constraints/solution/template.rs | 0 | 356 | 13,629 | 0 | One structural projection algorithm for replacement lifting, root closure, and binder transfer |
| arcweft-lang-sema/src/types/constraints/solution/tests.rs | 0 | 347 | 12,198 | 0 | Tests follow the completed-solution and instantiation boundary |
| arcweft-lang-sema/src/types/constraints/solution/residual.rs | 0 | 325 | 12,440 | 0 | Existing residual quantifier owner exposes slot lookup; no second origin inventory |
| arcweft-lang-sema/src/types/constraints/tests.rs | 3,868 | 3,973 | 138,453 | 0 | Existing transaction differentials adopt scoped/inference APIs; no production state added |
| arcweft-lang-sema/src/callable/checked_application.rs | 3,641 | 4,531 | 168,405 | 0 | Checked application owns the distinct template/result/source projection entry points |
| arcweft-lang-sema/src/callable/continuation.rs | 2,307 | 2,383 | 92,825 | 27 | Existing callable invariant algebra propagates typed instantiation failures |
| arcweft-lang-sema/src/callable/join.rs | 859 | 1,758 | 66,733 | 0 | Owns runtime selection and transcript; layer history is replaced by a shared flat lower environment |
| arcweft-lang-sema/src/final_analysis/analyzer/call_seal.rs | 1,542 | 1,979 | 87,415 | 0 | Existing publication owner selects template versus completed-source projection |
| arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs | 4,241 | 4,944 | 202,177 | 476 | Candidate owner transfers the scoped result; no alternate result resolver was added |
| arcweft-lang-sema/src/final_analysis/model.rs | 2,481 | 2,774 | 90,003 | 0 | Existing method-selection admission handles fallible join identity |
| arcweft-lang-sema/src/final_analysis/semantic_transcript.rs | 1,193 | 4,098 | 164,099 | 98 | Existing transcript writer propagates join scope errors |
| arcweft-lang-sema/src/final_analysis/tests.rs | 8,292 | 8,993 | 310,246 | 0 | Two existing join identity assertions migrate; no unrelated test family was added |
| arcweft-compiler/src/lower.rs | 3,981 | 7,754 | 328,467 | 0 | Existing semantic/runtime projection owner uses fallible flat environments and the already-closed callable ABI |

The new instantiation owner is 215 LOC / 8,332 bytes. It shares the structural
walker and binding row types with completed solution projection; the active
solution's continuation metadata does not become runtime state. The compiler
instance graph owner is 216 LOC / 7,262 bytes and lost its duplicate ABI field.
Other touched files are the solution/constraint facade, normalization,
substitution, callable application, and compiler variant projection; their
metrics are retained in the linked report. Production changes stay in the
existing type, callable, final-analysis, and compiler ownership direction.
No dependency, feature, host I/O, transport, or persistence surface changed.
Workspace fan-in/out is sema 8/14 (development 3/0), compiler 3/23
(development 1/5). The large owners retain their established responsibilities;
physical extraction of these call-site/error changes would add an artificial
boundary rather than separate state. The full cut still requires broader
structure and execution validation.

## Remaining work and design status

The immediate missing owner is anonymous function-scheme parameter inventory.
The current callable inventory and completed binding keys require declaration
IDs. `for_function_value` must admit the root function binder through real
scheme slots, with fresh application openings and scope-preserving producers
and consumers. Ignoring `binder`, manufacturing declaration IDs, or rejecting
nonempty binders would conceal that missing boundary and is not adopted.

The [active correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
still covers that inventory, effect namespaces, contextual/generic guard
selection, monomorphic specialization evidence, discovery limits and
cancellation, and unified callable execution through suspension/restore.
The runtime compiler edits above have not type-checked past their sema
dependency and do not establish native/AWBC acceptance. Match, View,
RuntimePlan, nominal, and scheduler/restore goal obligations are unchanged.
No returned design was accepted, no compatibility path or language-wide
scheme ban was introduced, and no final goal acceptance is claimed. There
is no external blocker; the full goal remains active.

The subsequent [function-scheme inventory record](2026-09-08-function-scheme-inventory.md)
supersedes this record's compiler and immediate inventory status. It records
successful sema/compiler checks, focused scope tests, and the still-failing
native/AWBC execution matrix without changing the earlier results above.
