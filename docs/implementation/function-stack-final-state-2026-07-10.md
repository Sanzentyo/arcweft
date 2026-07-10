# Function stack final state — 2026-07-10

This note is the authoritative completion map for the revised
function/closure/currying/pipeline goal. It supersedes the current-state and
gap summaries dated 2026-07-09; those files remain as historical audit
snapshots.

## Completed language contract

- Function types use right-associative `A -> B`; one tuple type denotes one
  multi-argument call group.
- `fn`, `task fn`, `dialogue fn`, `stream fn`, trait members, and impl members
  preserve multiple authored `ParamGroup`s. A `flow` accepts at most one group
  and rejects curried parameter spelling.
- `f(a)(b)` is staged application and remains distinct from the single call
  group `f(a, b)` through syntax, HIR, sema, runtime-plan, and AWBC lowering.
- Closures support `|x| expr`, `|| expr`, typed parameters, destructuring
  patterns, block return annotations, deterministic capture inventory, and
  borrowed-capture diagnostics at suspension boundaries.
- Expression `_` creates one partial-application abstraction region and is not
  the pattern wildcard. Pipe placeholder `^` exists only in a `|>` RHS.
- `|>` associates left. A RHS containing `^` substitutes the evaluated left
  value at its placeholder sites; a RHS without `^` uses data-last
  application. Method syntax resolves real inherent/environment/trait methods
  before the typed data-last callable fallback and reports ambiguity rather
  than choosing by source order.
- Let type ascription, unsuffixed numeric inference and fallback, LSP inlays,
  and numeric lint policy are implemented without requiring literal suffixes.
- Primitive source names are canonical. Removed spellings are rejected rather
  than accepted through aliases or formatter normalization. `Unit`, `Never`,
  relative runtime IDs, and expected-type enum shorthand use the same typed
  paths as the rest of the language.

## Completed executable contract

- Runtime functions support capture, exact apply, partial apply, curried
  apply, returned functions, and accepted source-local function candidates.
- AWBC `MakeFunction` / `ApplyFunction` uses the ordinary Fiber call stack.
  Dynamic apply now preserves exact caller state across await, host calls,
  explicit yield, and budget preemption.
- Product session snapshots serialize and validate AWBC-backed captured and
  partially applied functions against the generation-pinned artifact.
  Structured-expression bodies and stale or malformed function identities are
  rejected; no provisional compatibility reader remains.
- Ordinary source `fn` values and analyzable closures receive fresh inferred
  effect-row variables. Curried body effects are attached to the final call
  group, higher-order callbacks retain delayed invocation evidence, and closed
  report projection remains the lower-layer boundary. LSP hover, inferred-let
  inlays, and effect trace diagnostics consume that owned evidence; raw `eN`
  inference variables are not exposed as ordinary user-facing type labels.
- Source, stream, flow-return, and host-request lowering is fail-closed.
  Unsupported statements, patterns, expressions, malformed policies, and host
  targets produce structured owner/path/role/source-range diagnostics instead
  of disappearing, becoming `Noop`, or being stringified into synthetic
  payloads.

## Deliberately separate contracts

One function-family question is genuinely underspecified by the revised
language brief: `task fn`, `dialogue fn`, and `stream fn` do not share ordinary
function creation/start/resume/cancellation semantics. Their curried syntax and
semantic call-group shape are implemented, but assigning ordinary open rows or
final-group execution timing would silently choose a task/dialogue/stream ABI.
The required execution table, runtime descriptors, save behavior, and tests are
isolated in:

```text
docs/reviews/requests/2026-07-08-seq-07.8.1-task-dialogue-stream-callable-effect-abi.md
```

This boundary is not a compatibility promise. The unreleased representation
may be replaced directly once that execution contract is decided.

First-class allocation of arbitrary host/adapter-backed callables and bound
method values is larger work than the revised language surface, which requires
method-call sugar but not escaped method values or adapter thunks. Unsupported
families continue to fail with structured diagnostics; the future runtime ABI
inventory remains in:

```text
docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md
```

Runtime ID atom-table storage is also not a language-completion item. Typed ID
paths are already in place; an atom table should be introduced only if current
checkout profiling shows a material comparison, hashing, serialization, or
allocation problem.

## Evidence

- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`
- `docs/implementation/function-stack-awbc-resume-function-snapshot-2026-07-10.md`
- `docs/implementation/function-stack-effect-row-curried-higher-order-timing-2026-07-09.md`
- `docs/implementation/function-stack-checked-executable-lowering-2026-07-10.md`
- `docs/01-language/functions-and-pipeline.md`

Validation for the final checked-executable and effect-row cut is recorded in
the two implementation notes above and in the final structural-audit report for
this sequence. `just test-workspace`, workspace all-target/all-feature check,
and changed-owner `-D warnings` clippy all pass on the final checkout.
