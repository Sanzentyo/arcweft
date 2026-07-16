# Lang-01.2 Stage 4 — entry-bound Agent artifacts

## Result

Agent controller artifacts now originate from one accepted
`CheckedAgentEntry` and its exact ordinary `HirFunction`. The final manifest
schema v1 identifies the selected entry and controller separately and carries
the checked binding, callable contract, policy, and budget digests.

The runtime runner validates those manifest fields against the bytecode entry
roles, callable executable, flow executable, and controller flow before it
constructs or starts an executor. Both bundle execution and direct bytecode
execution start an explicit Agent entry; neither path infers a first entry or
accepts a non-Agent entry.

## Typed lowering evidence

Project-function evidence retains its existing global expression ID and also
records:

- the exact `CallableDeclarationId`;
- the expression ID relative to that function body.

Each requested Agent controller lowers with a fresh owner-scoped expression
cursor. Unbound ordinary functions before or between controllers therefore
cannot shift numeric, function-value, or method-resolution evidence into a
different controller.

## Verification

Focused checks completed:

```text
cargo test -p arcweft-agent-protocol artifact::tests
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler
cargo test -p arcweft-compiler --lib project::entry_tests::agent_controller_uses_callable_local_type_evidence_after_unbound_function
cargo test -p arcweft-compiler --lib project::entry_tests::each_agent_controller_restarts_evidence_at_its_exact_callable_body
cargo test -p arcweft-compiler --lib project::entry_tests::ordinary_agent_entry_round_trips_and_runs_with_exact_artifact_binding
cargo test -p arcweft-agent-runner controller_bundle
cargo test -p arcweft-agent-runner controller_bytecode
cargo test -p arcweft-agent-runner controller_bundle_rejects_tampered_entry_bound_manifest_fields
cargo test -p arcweft-agent-runner controller_bytecode_rejects_explicit_non_agent_entry_before_execution
```

The end-to-end regression compiles an ordinary function plus `entry agent`,
serializes and decodes the final bundle, executes the decoded artifact through
`AgentRunner`, and rejects an in-memory binding-hash tamper before execution.

## Next stage

Stage 5 removes the predecessor Agent-item source/HIR/compiler APIs and migrates
CLI, REPL, samples, and tests to explicit project entry selection. No
compatibility reader or dual artifact path is retained.
