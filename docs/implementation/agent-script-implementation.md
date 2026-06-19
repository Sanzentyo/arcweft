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
- Agent intrinsic typing includes `expect(condition, message?)`, `deny(condition, message?)`, `signal(ref) -> Probe<T>`, `metric(ref) -> Probe<T>`, `Probe<T>.eq(T) -> Predicate`, `wait(predicate, timeout=...) -> Observation` with timeout required and literal `stable_frames`/`poll_frames` values required to be at least 1, `choose(Ref<ChoiceOption>) -> ActionResult`, `invoke(target, action, args?) -> ActionResult` against project-index Agent action signatures with named payload parameter validation, `capture(viewport|layer|object, ...) -> CaptureRef`, `attach(CaptureRef)`, `checkpoint(String)`, `note(DisplayText)`, and `rag.query(String, roots=..., graph_depth=..., limit=...) -> RagContextPack`.
- `arcweft-compiler` exposes `compile_agent_source_with_project`, which resolves `.awfagent` entity references against the module plus `ProjectSemanticIndex` and returns a `TypeCheckReport`.
- `arcweft-runtime-plan` can lower a typed single Agent controller body into the shared runtime-plan shape, and `arcweft-compiler` exposes `compile_agent_bundle_with_project` to produce an Agent controller `.awfb` data object with bytecode, embedded source, and `AgentArtifactManifest`.
- `arcweft-agent-protocol` owns Agent controller contract modules for artifact manifests, stable IDs, predicates, host requests/responses, traces, and typed values. Large host request/response enum payloads are boxed so the Rust protocol API does not carry oversized enum variants while preserving the same serde JSON shape.
- `arcweft-debug-model` provides Sans-I/O debug events, chunks, embedding records, RAG query models, and debug sink boundaries.
- `arcweft-rag` provides deterministic exact vector ranking and reciprocal-rank fusion primitives.
- `arcweft-agent-runner` provides the `AgentSession` host boundary, deterministic runtime policy checks, bounded wait polling with stable-frame confirmation, debug event emission, and a RAG service boundary for controller host calls. It can now run Agent controller bytecode through the shared core bytecode VM, dispatch effect-form Agent calls such as `observe(...)`, `checkpoint(...)`, `choose(...)`, `capture(...)`, `read_resource(...)`, and `rag.query(...)`, and bridge Agent `HostTaskRequest::Custom` suspend/resume tasks back into the VM so expressions such as `let shot = capture(...)` can bind the returned capture record and continue execution.
- `arcweft-bundle` can now encode `.awfb` bundles as `bundle_kind = agent_controller` with an embedded `AgentArtifactManifest`, while ordinary game bundles remain the default. The game `arcweft-runtime-host` bundle runner rejects Agent controller bundles instead of executing them as game bytecode.
- `arcweft-debug-sqlite` provides the rebuildable `SQLite`/FTS5 debug index, event sink adapter, Japanese lexical smoke coverage, and little-endian f32 vector blob storage without unsafe casts.
- `arcw agent script check <file.awfagent>` validates Agent dialect parsing/HIR lowering without requiring the `native-capture` feature.
- `arcw debug db status|migrate` opens and migrates the rebuildable Agent debug `SQLite` database at `.arcweft/cache/agent-debug.sqlite3` by default.
- `samples/agent-script/opening-smoke.awfagent` and `samples/agent-script/visual-regression.awfagent` mirror the package examples and currently pass `agent script check`.

## Deliberate boundaries

- Agent Script reuses the existing expression, statement, block, attribute, contract/effect, and HIR lowering path.
- No compatibility shim for the old line-command syntax is present.
- `compile_agent_source` remains the parser/HIR-only entry point for lightweight syntax checks; `compile_agent_source_with_project` is the typed project-index entry point; `compile_agent_bundle_with_project` is the typed artifact entry point. Controller VM execution and host-call dispatch remain separate follow-up work.
- Agent source defaults such as omitted signature return type are preserved as syntax/HIR shape for later semantic/compiler resolution rather than being string-rewritten in the parser.
- `invoke` is checked as a semantic action contract on a typed project entity. It does not accept string target IDs, does not fall back to physical actions, and validates payload record keys and values against named project-index Agent action parameters.
- `arcweft-agent-contract-reference` was not added as a production crate. Its concepts were merged into `arcweft-agent-protocol` modules to avoid duplicate protocol surfaces.
- `arcweft-debug-model` and `arcweft-rag` are Sans-I/O crates. They do not open databases, read files, call embedding services, or inspect runtime state directly.
- `arcweft-agent-runner` executes typed host requests and can step Agent controller bytecode for both effect-form calls and suspended Agent host-call expressions. It bridges `agent` custom host tasks to the same `AgentHostRequest` boundary and resumes the VM with a typed record payload. `wait(...)` expression lowering still needs a structured `Predicate` payload path before it can be rebound through the same task bridge; direct `AgentHostRequest::Wait` remains implemented at the runner boundary.
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
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_body_to_entry_flow -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_bundle -- --nocapture`
- `cargo check -p arcweft-runtime-plan -p arcweft-compiler`
- `cargo test -p arcweft-compiler -- --nocapture`
- `cargo clippy -p arcweft-agent-protocol --all-targets --all-features -- -D warnings`
- `cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-agent-protocol -p arcweft-agent-runner`
- `cargo test -p arcweft-agent-protocol -p arcweft-agent-runner`
- `cargo test -p arcweft-agent-protocol -p arcweft-runtime-plan -p arcweft-agent-runner`
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_host_call_let_to_await -- --nocapture`
- `cargo test -p arcweft-agent-runner controller_bytecode_dispatches_effect_calls_to_runner_host_boundary -- --nocapture`
- `cargo test -p arcweft-agent-runner controller_bytecode_resumes_bound_capture_response -- --nocapture`
- `cargo test -p arcweft-agent-runner controller_bundle_runs_through_bytecode_host_boundary -- --nocapture`
- `cargo clippy -p arcweft-agent-protocol -p arcweft-runtime-plan -p arcweft-agent-runner --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-bundle -p arcweft-runtime-host`
- `cargo test -p arcweft-bundle -p arcweft-runtime-host bundle -- --nocapture`
- `cargo clippy -p arcweft-bundle -p arcweft-runtime-host --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-cli -p arcweft-player-native -p arcweft-runtime-host`
- `cargo check -p arcweft-desktop-native --all-features`
- `cargo test -p arcweft-desktop-native --all-features`
- `cargo clippy -p arcweft-desktop-native --all-targets --all-features -- -D warnings`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/opening-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/visual-regression.awfagent --json`
- `cargo run -p arcweft-cli -- debug db status --path target/codex-agent-script-final/agent-debug-test.sqlite3 --json`

## Other platforms

- Linux/macOS should run the same check/test/clippy commands above.
- No Linux/macOS runtime validation has been performed in this workspace yet.

## Remaining zip-derived work

- Type Agent references against actions and resources beyond the current choice/layer/signal/metric project-index coverage.
- Add structured `Predicate` lowering for `wait(...)` expression tasks so the suspend/resume bridge can execute rebound waits without string reconstruction.
- Persist runner debug events as `.arcwx` trace artifacts from CLI/MCP run and replay commands.
- Extend MCP/CLI with script run/replay, action dispatch, wait, REPL, debug search, and RAG commands using the shared JSON/resource shapes.
- Add privacy classification enforcement, debug-store reindex/delete validation, CLI/MCP debug commands, and RAG explain surfaces.
- Add end-to-end Windows validation once script run/replay and CLI/MCP commands exist.
