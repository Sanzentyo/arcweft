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

## Remaining Before Goal Completion

These are the remaining items that still affect the active Agent Script final
goal:

- Re-run a compact milestone validation set after the latest lifecycle
  privacy cut, including script source/bundle run parity, debug DB lifecycle
  readback, RAG readback, and at least one native Agent Script scenario.
- Refresh `agent-script-implementation.md` so the remaining section separates
  package-goal leftovers from broader product-player work. In particular,
  product-wide graph ownership and native/player-daemon scheduling are beyond
  the package's initial Agent Script acceptance criteria unless a later design
  explicitly expands this goal.
- Run one final search for stale Agent Script package syntax and stale
  `*.awfagent.ndjson` references before marking the goal complete.

## Explicit Non-Goals For This Package

- A long-lived product player daemon and real owned-window scheduling are
  product runtime work, not required to prove the Agent Script package's
  controller VM, CLI/MCP, trace, debug store, and RAG acceptance criteria.
- Remote REPL endpoints such as `stdio:` and `mcp:` are intentionally rejected
  until a concrete transport design exists. Local source/profile/trace REPL
  behavior is the implemented package scope.
- ANN vector search is not required for v1; exact normalized `f32` ranking is
  the specified baseline.
