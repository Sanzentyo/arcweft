# Unary Need atomic boundary implementation — 2026-08-21

Supersedes the implementation status in
[`2026-08-19-unary-need-atomic-boundary-audit.md`](2026-08-19-unary-need-atomic-boundary-audit.md).

## Inspected state

- Base Git revision: `4c4554f9cd6fa45110ea46199bca47b2fa3fbdfc`
  (`Implement explicit extension receivers`).
- Branch: `main`, initially clean and matching `origin/main`.
- Working tree while this record was written: dirty with the coherent unary
  Need implementation cut described below.
- Maintained design authority:
  [`Await, unary Need, carrier blocks, and try`](../01-language/await-need-result.md).

## Implemented result

The temporal carrier now has one payload authority throughout the checked and
runtime stacks:

1. `arcweft_need::Need<T>` owns only `NotStarted`, `Pending`, `Ready(T)`, and
   `Cancelled`. The obsolete error coordinate, `Err` state, and `map_err`
   behavior were deleted.
2. Semantic, environment, adapter, RuntimePlan, compiler, codec, verifier, and
   tooling type projections now represent `Need<T>` with one item coordinate.
   Fallible registered producers publish `Need<Result<T, E>>` using their
   existing typed Result and nominal error identities.
3. Await checks and lowers as `Need<T> -> T`. The synthetic physical Result,
   Await Ready/Error/Denied observer kinds, branch payload copies, and checked
   continuation-result copies were deleted. Multiple source-ordered Pending
   observers remain because the maintained Activity chapter still admits that
   cardinality.
4. Pending observer patterns are typed from the Arcweft-owned `Progress` value
   family. `Progress` has dedicated semantic, RuntimePlan, AWBC, runtime value,
   canonical/save-codec, ownership, shape, and field-projection support. Its
   owning field schema exposes `ratio: f32` and `label: Option<String>`; no
   `Named("Progress")`, dynamic fallback, or copied nominal side table was
   introduced.
5. Structured and Product AWBC Ready paths resume with the admitted payload
   unchanged. A Result payload remains an ordinary value-level Result rather
   than a second temporal error channel. Existing codecs evolved in place and
   every touched Arcweft version marker remains `1`.

## Validation performed

### Passed

- `cargo fmt --all`
- `git diff --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo test -p arcweft-need`: 3 passed.
- `cargo test -p arcweft-lang-sema --lib`: 204 passed, including the new
  Progress-field owner test.
- `cargo test -p arcweft-lang-syntax --lib`: 669 passed.
- `cargo test -p arcweft-lang-hir --lib`: 829 passed, 2 failed, 8 ignored on
  the first run. Both failures were stale Await Ready/Error/Denied test rows;
  after migrating them, each exact failed test passed.
- `cargo test -p arcweft-runtime-plan --lib`: 47 passed.
- Focused adapter-context, adapter-sema, compiler, and runtime-plan suites:
  130 tests passed in total.
- Direct compiler and verifier checks passed for existing Await fixtures 010,
  011, and 014, run fixtures 002 and 003, benchmark fixture 004, and the new
  Pending/Progress field fixture 055.
- `cargo clippy --workspace --all-targets --all-features`: exit status 0. The
  workspace still reports pre-existing advisory warnings, including large
  owner functions and files; no Clippy-denied failure was hidden.
- `just test-doc`: passed; the workspace doc suites that contain tests ran 4,
  2, and 2 tests successfully.
- `just structure-audit` and `just structure-audit-gate`: passed with 0
  blocking violations.
- Tier 2 follow-up after the aggregate recipe stopped:
  - MCP slow tests: 4 passed.
  - Select production boundaries: 5 of 6 passed. The one failure is recorded
    below.
  - Flow production boundaries: 2 of 2 passed.

### Failed during focused exploration

- Existing positive fixture 049 still fails with `ValueResolutionFailed` for
  `ExprId(23)`. The dedicated fixture 055 validates the new Progress owner and
  both field projections, so 049 is not used as unary Need acceptance evidence.
- Existing Result fixture 042 still fails at the pre-existing shared callable
  resolution boundary.
- `just test-tier2` stopped in `test-slow-agent-observe` after the MCP tests
  passed. The native observation fixture reaches the pre-existing executable
  dialogue projection failure: no selected
  `localization.character_names` policy.
- Individual continuation of `test-native-aux-capture` stopped at its first
  case because `samples/image-animation.arcw` still uses rejected top-level
  image and character-member syntax.
- Individual continuation of `test-visual-golden` stopped in its prerequisite
  visual smoke tests at the same executable dialogue localization boundary.
- Select Tier 2's
  `e13_tier2_total_slots_one_over_rolls_back_the_direct_select` failed after
  206.15 seconds at the unrelated HIR transaction assertion
  `retired prefill revision stays current`. The other five Select production
  boundary tests passed.

### Blocked by environment resources

- `just test-workspace` reached compilation but failed when Windows could not
  mmap several generated `.rlib` files because the paging file was too small
  (OS error 1455). Focused suites and the full workspace check passed; the
  aggregate test result is not claimed as passed.

## Structural review

The structural audit reports review triggers but no blocking dependency
violation. The touched large owners remain cohesive at their established
boundaries: adapter codecs own closed adapter encoding; semantic environment,
expression, preparation, and validation modules own typed analysis; compiler
and RuntimePlan modules own their respective exhaustive projections; core
construction, pure evaluation, AWBC verification, and AWBC execution modules
own runtime lowering and behavior; and `cli_runtime_bench` remains the
integration fixture matrix. This cut deletes obsolete coordinates and adds
exhaustive Progress arms inside those existing responsibilities. It does not
introduce cross-layer state or a second type authority, so splitting an owner
solely to reduce its line count would not improve the dependency or API
boundary established by this cut.

## Remaining work and explicit non-goals

- Producer outcome classification remains a later cut. Host task completion
  still bridges the current outcome contract; this cut only establishes that
  temporal Ready carries one admitted payload.
- Pending observer runtime scheduling and re-wait behavior are not claimed as
  complete here. This cut establishes the typed observer and canonical
  Progress substrate.
- Stream migration and timeout combinators are not part of this cut.
- The unrelated fixture failures recorded above are not repaired here.

## Design precedence

The maintained Activity chapter currently shows multiple source-ordered
Pending observer arms. The final HIR and checked representation therefore use
an observer list rather than collapsing cardinality to `Option`. Every observer
is implicitly Pending and is typed from canonical `Progress`; the deleted kind
enum is not retained for forward compatibility.
