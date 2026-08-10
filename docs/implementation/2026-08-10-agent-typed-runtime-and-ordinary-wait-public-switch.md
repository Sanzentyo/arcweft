# Agent typed runtime and ordinary-wait public switch

Date: 2026-08-10

Inspected Git commit: `e9a89ec410a5f1573530ab8f47638329610d0736`

Working-tree state at inspection: dirty on `main` with this coherent language,
semantic, compiler, runtime-plan, runner, sample, and validation-recipe cut. The
sole repository worktree and normal shared Cargo target were used. Cargo
validation used four build jobs and normal incremental compilation.

## Performed

Selected Agent Prelude calls now cross the final semantic/runtime boundary by
closed typed identity. The compiler exhaustively projects every
`AgentIntrinsicSignatureId` into `RuntimeAgentIntrinsic`; effectful operations
become typed Agent host tasks, while deterministic targets, probes, predicates,
and action descriptors become structured runtime expressions. Positional,
named, and spread arguments retain their evaluated order at the host request.
No source spelling, HIR name scan, generic Core intrinsic, or legacy stringly
effect request selects the operation.

Probe comparisons on arbitrary receiver expressions now retain one typed chain:

```text
final HIR Call(Value(Select(receiver, member)))
  -> selected DomainMethodId::ProbeCompare
  -> canonical ProbeComparisonOperator
  -> RuntimeAgentProbeComparison
  -> { kind: "compare", probe, op, value }
```

The selected method fact owns the receiver type. A bound method expression used
outside its checked Call fails closed at runtime lowering rather than becoming a
field or a source-derived callable.

Agent protocol records expose their fields from `TypeKind` ownership, including
the exact `ActionResult.accepted` boolean contract. Capture targets are accepted
only through their typed runtime record shape; the old string fallback is
deleted. Capture format/kind and pointer-button parameters are registered as
their closed semantic enum types. Open Agent action names retain the distinct
`ActionName` type and are not restricted to pointer-button cases.

Ordinary function bodies now parse direct `wait(...)` as an ordinary Call
expression. Flow bodies retain their dedicated `WaitStatement` family. The
obsolete `LetAwaitStatement` syntax/attachment/formatter surface is deleted;
`let value = await ...` and `let value = try ...` use an ordinary Let whose
initializer owns the expression. Candidate and unbound ordinary statement
tests no longer preserve a removed Wait-recovery diagnostic.

Final-HIR pattern locals now retain the exact authored binding span for record
and sequence rest bindings. They no longer publish a zero-width insertion at
the end of the rest pattern.

The Proof validation recipes use ordinary Cargo incremental behavior. The
temporary `CARGO_INCREMENTAL=0` wrapper introduced for multiple targets is
deleted.

## Validation

Passed with `--jobs 4` unless the command is a non-Cargo audit:

- `cargo test -p arcweft-lang-syntax`: 691 unit tests, public API closure,
  parser authority, and doc tests passed;
- `cargo test -p arcweft-lang-hir`: 857 unit tests passed, 8 ignored, with
  public API/compile-fail/doc coverage passed;
- `cargo test -p arcweft-lang-sema`: 188 unit tests and its integration,
  compile-fail, and doc coverage passed;
- `cargo test -p arcweft-runtime-plan`: 24 unit tests plus API closure,
  assertion identity, 59 AWBC parity cases, iterator witness, and docs passed;
- `cargo test -p arcweft-agent-runner`: 45 tests passed;
- focused compiler runtime-request test
  `selected_agent_controller_lowers_typed_probe_comparison_into_wait_request`:
  passed and asserted `all -> not -> compare(op = eq)` in the final Agent wait
  request;
- `cargo check --workspace --all-targets --all-features --jobs 4`: passed;
- `cargo clippy --workspace --all-targets --all-features --jobs 4 --
  -D warnings`: passed;
- `cargo fmt --all -- --check` and `git diff --check`: passed;
- `just structure-audit-gate`: 2,128 files, 2,005 Rust files, 991,429 Rust
  physical LOC, 94 workspace packages, 182 review triggers, and zero blocking
  violations; and
- Tier 2 `agent_mcp_stdio_runs_agent_script` with `--ignored --exact`: one
  passed; the test body completed in 2.36 seconds.

The requested `cargo clean` had already removed 281.7 GiB of regenerable
artifacts before this validation. The validation above rebuilt the ordinary
shared target with four jobs. No second clean was run, because it would only
discard that newly shared incremental state. `git worktree list` reports the
single `D:/git/arcweft` worktree.

## Non-green broader rows

`cargo test -p arcweft-compiler --jobs 4` passed all 51 unit tests, including
the new Agent end-to-end test, and its API closure. The command remains
non-green in the independent `view_product` integration target: one test passes
and six fail. The failures expose the already selected next View/RichText
boundary: `Image` has no checked callable, accepted View items lack a checked
runtime product projection, and several tests retain old diagnostic
stage/code/cardinality expectations. This cut does not add a temporary `Image`
builtin, restore the retired View lowerer, or weaken those tests.

The previously executed full slow MCP row remains 5 passed and 17 failed. The
exact Agent script row above is green; the remaining failures enter player
initialization and typed RichText/sample admission. They are assigned to the
next AW-AH-007/008 typed RichText slice instead of being repaired with fixture
shims or source fallbacks.

## Request dispatch guidance

No design request is dispatched from this cut. The exposed failures belong to
returned, implementation-ready typed RichText/View work, not to a missing Agent
runtime contract.

The three supplied Lang-01.5.1 return archives must first be verified and
entered through the repository package ledger before implementation; they must
not be dispatched again merely because the earlier conversation classified
them as unreturned. Likewise, existing repository evidence says the
Lang-01.1.1.2 and retained Lang-01.3 correction cohorts require ledger/current-
`main` adjudication, not a duplicate request.

If a genuinely missing correction is confirmed after that audit, use its
existing narrow file under `docs/reviews/requests/`. Send that request together
with every parent/previous return named by its dispatch contract to one
design-only assignee. Explain the precise compiler-exposed boundary and require
one returned ZIP with `OPEN_QUESTIONS=0`, exact typed owner/ABI/codec/save
allocations where applicable, full producer/consumer/deletion matrices, and no
code overlay. Do not split one serialized boundary among independent requests,
and do not implement an inferred shape while waiting.

## Explicit non-goals

- no CSS or Takumi path;
- no compatibility alias, dual reader, source gate, source reconstruction, or
  shim;
- no removed-syntax-only diagnostic;
- no guessed Stream, typed-resource, manifest-slice, View, codec, or save shape;
  and
- no implementation of the supplied Lang-01.5.1 packages before verified
  package intake.
