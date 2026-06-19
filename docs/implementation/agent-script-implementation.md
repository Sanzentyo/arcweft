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
- `arcweft-lang-sema` exposes `project_index`, including `ProjectSemanticIndex`, typed entity payload metadata, Agent intrinsic lowering tags, and an Agent Prelude projection into `TypeCheckEnv`.
- `TypeKind::Ref` now carries `EntityType`, preserving an entity family and optional payload type through project-index projection into `TypeCheckEnv`.
- The type checker now visits Agent item bodies and can check Agent Prelude calls such as `choose(@choice...)` against project-index entity types and declared Agent effects.
- `arcweft-compiler` exposes `compile_agent_source_with_project`, which resolves `.awfagent` entity references against the module plus `ProjectSemanticIndex` and returns a `TypeCheckReport`.
- `arcweft-agent-protocol` owns Agent controller contract modules for artifact manifests, stable IDs, predicates, host requests/responses, traces, and typed values.
- `arcweft-debug-model` provides Sans-I/O debug events, chunks, embedding records, RAG query models, and debug sink boundaries.
- `arcweft-rag` provides deterministic exact vector ranking and reciprocal-rank fusion primitives.
- `arcweft-agent-runner` provides the `AgentSession` host boundary, deterministic runtime policy checks, bounded wait polling with stable-frame confirmation, debug event emission, and a RAG service boundary for controller host calls.
- `arcweft-debug-sqlite` provides the rebuildable `SQLite`/FTS5 debug index, event sink adapter, Japanese lexical smoke coverage, and little-endian f32 vector blob storage without unsafe casts.
- `arcw agent script check <file.awfagent>` validates Agent dialect parsing/HIR lowering without requiring the `native-capture` feature.
- `arcw debug db status|migrate` opens and migrates the rebuildable Agent debug `SQLite` database at `.arcweft/cache/agent-debug.sqlite3` by default.
- `samples/agent-script/opening-smoke.awfagent` and `samples/agent-script/visual-regression.awfagent` mirror the package examples and currently pass `agent script check`.

## Deliberate boundaries

- Agent Script reuses the existing expression, statement, block, attribute, contract/effect, and HIR lowering path.
- No compatibility shim for the old line-command syntax is present.
- `compile_agent_source` remains the parser/HIR-only entry point for lightweight syntax checks; `compile_agent_source_with_project` is the typed project-index entry point. Bytecode artifact lowering, controller VM execution, and host-call dispatch remain separate follow-up work.
- Agent source defaults such as omitted signature return type are preserved as syntax/HIR shape for later semantic/compiler resolution rather than being string-rewritten in the parser.
- `arcweft-agent-contract-reference` was not added as a production crate. Its concepts were merged into `arcweft-agent-protocol` modules to avoid duplicate protocol surfaces.
- `arcweft-debug-model` and `arcweft-rag` are Sans-I/O crates. They do not open databases, read files, call embedding services, or inspect runtime state directly.
- `arcweft-agent-runner` currently executes typed host requests. It does not yet start or step the shared bytecode VM; the VM will call this host-call boundary.
- `arcweft-debug-sqlite` is the only new I/O crate in this slice. It owns `rusqlite` and keeps database access out of syntax, HIR, compiler, runner, protocol, debug-model, and RAG crates.

## Windows validation

- `cargo check -p arcweft-debug-model -p arcweft-rag -p arcweft-debug-sqlite`
- `cargo test -p arcweft-debug-model -p arcweft-rag -p arcweft-debug-sqlite`
- `cargo clippy -p arcweft-debug-model -p arcweft-rag -p arcweft-debug-sqlite --all-targets --all-features -- -D warnings`
- The `arcweft-debug-sqlite` tests were run on Windows and validate migration, FTS5 Japanese search, and embedding blob round trips.
- `cargo fmt --check`
- `cargo check -p arcweft-cli -p arcweft-lang-syntax`
- `cargo check -p arcweft-cli --features native-capture`
- `cargo test -p arcweft-lang-syntax agent_dialect -- --nocapture`
- `cargo clippy -p arcweft-cli -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-lang-sema -p arcweft-compiler`
- `cargo test -p arcweft-lang-sema project_index -- --nocapture`
- `cargo test -p arcweft-lang-sema typecheck -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_source_with_project -- --nocapture`
- `cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/opening-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/visual-regression.awfagent --json`
- `cargo run -p arcweft-cli -- debug db status --path target/codex-agent-script-final/agent-debug-test.sqlite3 --json`

## Other platforms

- Linux/macOS should run the same check/test/clippy commands above.
- No Linux/macOS runtime validation has been performed in this workspace yet.

## Remaining zip-derived work

- Complete Agent semantic intrinsic rules for `observe`, `invoke`, `wait`, `capture`, `expect`, `deny`, `checkpoint`, `attach`, `note`, and `rag.query`, including structured diagnostics and policy denial.
- Type Agent references against actions, probes, metrics, signals, and resources using the payload-bearing entity refs.
- Add Agent artifact manifest / bundle support for `bundle_kind = agent_controller`.
- Connect `arcweft-agent-runner` to shared bytecode VM execution and `.arcwx` trace emission.
- Extend MCP/CLI with script run/replay, action dispatch, wait, REPL, debug search, and RAG commands using the shared JSON/resource shapes.
- Add privacy classification enforcement, debug-store reindex/delete validation, CLI/MCP debug commands, and RAG explain surfaces.
- Add end-to-end Windows validation once script run/replay and CLI/MCP commands exist.
