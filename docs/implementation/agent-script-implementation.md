# Agent Script implementation status

This note tracks the implementation of `arcweft-agent-script-final-2026-06-18.zip`.
It is implementation state, not the stable language specification.

## Current cut point

- `.awfagent` is represented as an Arcweft Agent dialect, not a separate line-command language.
- The syntax parser exposes `SourceDialect`, `ParseOptions`, `parse_document`, and `parse_fragment`.
- `SourceDialect::Agent` accepts top-level `agent` items and rejects raw top-level fallback, so legacy forms such as bare `observe` do not parse as Agent Script.
- `agent @agent.id name(...) effects { ... } { ... }` is preserved as `Item::Agent`.
- HIR preserves Agent controllers in `HirModule::agents()` as `HirAgent`.
- `arcweft-compiler` exposes `parse_agent_source_text` and `compile_agent_source`, which parse Agent dialect source and lower it through the shared HIR path.
- `arcweft-agent-protocol` owns Agent controller contract modules for artifact manifests, stable IDs, predicates, host requests/responses, traces, and typed values.
- `arcweft-debug-model` provides Sans-I/O debug events, chunks, embedding records, RAG query models, and debug sink boundaries.
- `arcweft-rag` provides deterministic exact vector ranking and reciprocal-rank fusion primitives.

## Deliberate boundaries

- Agent Script reuses the existing expression, statement, block, attribute, contract/effect, and HIR lowering path.
- No compatibility shim for the old line-command syntax is present.
- `compile_agent_source` currently stops at parser/HIR validation. Bytecode artifact lowering, controller VM execution, and host-call dispatch remain separate follow-up work.
- Agent source defaults such as omitted signature return type are preserved as syntax/HIR shape for later semantic/compiler resolution rather than being string-rewritten in the parser.
- `arcweft-agent-contract-reference` was not added as a production crate. Its concepts were merged into `arcweft-agent-protocol` modules to avoid duplicate protocol surfaces.
- `arcweft-debug-model` and `arcweft-rag` are Sans-I/O crates. They do not open databases, read files, call embedding services, or inspect runtime state directly.

## Remaining zip-derived work

- Add Agent semantic intrinsics and type/effect checks for `observe`, `choose`, `invoke`, `wait`, `capture`, `expect`, `deny`, `checkpoint`, `attach`, `note`, and `rag.query`.
- Add `ProjectSemanticIndex` and type Agent references against project entities, actions, probes, metrics, signals, and resources.
- Add Agent artifact manifest / bundle support for `bundle_kind = agent_controller`.
- Add `arcweft-agent-runner` with `AgentSession`, bounded wait state machine, debug event sink, RAG service boundary, and `.arcwx` trace emission.
- Extend MCP/CLI with script run/replay, action dispatch, wait, REPL, debug search, and RAG commands using the shared JSON/resource shapes.
- Add SQLite/FTS5 debug store, privacy classification enforcement, and reindex/delete validation.
- Document Windows validation, plus Linux/macOS validation procedure and current status, once runner/CLI behavior exists.
