# Agent Script Final Completion Audit

This audit maps `arcweft-agent-script-final-2026-06-18.zip` to the current
Arcweft implementation. It is implementation state, not a stable language
chapter.

## Package Requirements

| Requirement | Current evidence | Status |
|---|---|---|
| `.awfagent` is an Arcweft Agent dialect, not a line-command DSL | `SourceDialect::Agent`, parser tests, old `wait signal`/bare top-level forms rejected, no stable docs match old line-command syntax | Covered |
| Agent source shares parser, HIR, sema, runtime plan, bytecode, and diagnostics | `compile_agent_source_with_project`, `compile_agent_bundle_with_project`, Agent HIR and Prelude type-checking, runner VM execution tests | Covered |
| Agent artifacts are `.awfb` bundles with `bundle_kind = agent_controller`; traces are `.arcwx` | `arcw agent script build/run/trace/replay`, bundle kind checks, trace validation and replay tests | Covered |
| Controller VM reaches observe, semantic action, wait, capture, resource read, debug record, RAG, and assertions through `AgentSession` | `arcweft-agent-runner` host-task dispatch, CLI/native Agent Script samples and focused tests listed in `agent-script-implementation.md` | Covered |
| REPL uses fragment parser/compiler/runner, not a second evaluator | `arcw agent repl`, `arcweft-tooling::agent_repl`, REPL cell persistence and snapshot rejection tests | Covered |
| Debug store is rebuildable SQLite/FTS/CAS state outside core | `arcweft-debug-model`, `arcweft-debug-sqlite`, `arcw debug db validate/reindex/prune/vacuum/delete`, CLI/MCP readback tests | Covered |
| RAG combines lexical, vector, graph, history, diagnostics, and tests with explainable selected context | `arcweft-rag`, CLI/MCP `rag.query`, `rag.explain`, `rag.context.read`, persisted `RagContextPack` audit tests | Covered |
| Remote embedding is default-deny and never uses synthetic vectors | `debug db embed --provider remote --remote-command ...`, remote-provider-unavailable diagnostics, provider privacy filtering tests | Covered |
| Privacy/capability/budget denial is structured and deterministic | runtime policy checks, manifest budgets, debug/RAG/resource max-privacy tests, lifecycle readback `max_privacy` gating | Covered for Agent Script/debug tooling paths |
| Windows validation exists for native Agent Script and debug/RAG paths | Windows commands and results are recorded in `agent-script-implementation.md` | Covered |
| Linux/macOS validation procedure is documented | `agent-script-implementation.md` lists other-platform status and commands | Covered |

## Completion Judgement

The package-scoped Agent Script final goal is implemented and validated against
the 2026-06-18 package requirements. Remaining implementation notes in
`agent-script-implementation.md` include broader product graph/player follow-up
that is not required for the package acceptance criteria unless a later design
explicitly expands this goal.

## Latest Compact Validation

Run on Windows in this workspace after the lifecycle privacy cut:

```bash
cargo run -p arcweft-cli -- agent script check samples\agent-script\opening-smoke.awfagent --json
cargo run -p arcweft-cli -- agent script check samples\agent-script\failure-investigation.awfagent --json
cargo run -p arcweft-cli -- agent script build samples\agent-script\cli-run-smoke.awfagent --output target\codex-agent-script-final-validation\cli-run-smoke.awfb --json
cargo run -p arcweft-cli -- agent script run samples\agent-script\cli-run-smoke.awfagent --json --trace-out target\codex-agent-script-final-validation\cli-run-smoke-source.arcwx
cargo run -p arcweft-cli -- agent script run target\codex-agent-script-final-validation\cli-run-smoke.awfb --json --trace-out target\codex-agent-script-final-validation\cli-run-smoke-bundle.arcwx
cargo run -p arcweft-cli -- agent script replay target\codex-agent-script-final-validation\cli-run-smoke-source.arcwx --expect target\codex-agent-script-final-validation\cli-run-smoke-bundle.arcwx --json
cargo test -p arcweft-cli --test check agent_script_run_persists_debug_session_and_script_run -- --exact --nocapture
cargo test -p arcweft-cli --test check agent_rag_query_uses_local_embedding_debug_db_channel -- --exact --nocapture
cargo test -p arcweft-cli --test check debug_db_embed_remote_command_indexes_provider_vectors -- --exact --nocapture
cargo run -p arcweft-cli --features native-capture -- agent script run samples\agent-script\native-choice-dispatch.awfagent --native-source samples\agent-script\native-choice-dispatch.arcw --json --trace-out target\codex-agent-script-final-validation\native-choice-dispatch.arcwx
cargo run -p arcweft-cli -- agent script trace target\codex-agent-script-final-validation\native-choice-dispatch.arcwx --json
```

Results:

- `opening-smoke.awfagent` and `failure-investigation.awfagent` checked with
  `ok = true`.
- Source and bundle execution of `cli-run-smoke` both completed with
  `final_status = Done(Return("done"))`, and replay reported
  `matched_expected = true`.
- Debug lifecycle, local embedding RAG, and remote command embedding tests
  passed.
- Native choice dispatch completed with `steps = 4`, `host_calls = 3`, one
  semantic action completion, and validated a trace with 11 records.
- A final stable-docs/source/sample search for old Agent line-command syntax
  and `*.awfagent.ndjson` references produced no matches. Historical review
  notes under `docs/reviews/` are not stable design contracts.

## Explicit Non-Goals For This Package

- A long-lived product player daemon and real owned-window scheduling are
  product runtime work, not required to prove the Agent Script package's
  controller VM, CLI/MCP, trace, debug store, and RAG acceptance criteria.
- Long-lived remote event streaming is not required for this package.
  Request/response MCP client endpoints such as `stdio:` are package scope and
  are tracked in `zip-gap-audit-2026-06-21.md`.
- ANN vector search is not required for v1; exact normalized `f32` ranking is
  the specified baseline.
