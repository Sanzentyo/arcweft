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
- `arcweft-lang-sema` exposes `project_index`, including `ProjectSemanticIndex`, typed entity payload metadata, Agent intrinsic lowering tags, HIR-to-project-index projection for checked native sources, Agent action signatures projected from typed inline presentation image calls, and an Agent Prelude projection into `TypeCheckEnv`.
- `TypeKind::Ref` now carries `EntityType`, preserving an entity family and optional payload type through project-index projection into `TypeCheckEnv`.
- The type checker now visits Agent item bodies and can check Agent Prelude calls such as `choose(@choice...)` against project-index entity types and declared Agent effects.
- Agent intrinsic typing includes `expect(condition, message?)`, `deny(condition, message?)`, `signal(ref) -> Probe<T>`, `metric(ref) -> Probe<T>`, `state(String) -> Probe<AgentValue>`, `observation(String) -> Probe<AgentValue>`, `Probe<T>.eq/ne/gt/ge/lt/le(T) -> Predicate`, `exists(Probe<T>)`, `all(...)`, `any(...)`, `not(...)`, `wait(predicate, timeout=...) -> Observation` with timeout required and literal `stable_frames`/`poll_frames` values required to be at least 1, `advance_text() -> ActionResult`, `choose(Ref<ChoiceOption>) -> ActionResult`, `invoke(target, action, args?) -> ActionResult` against project-index Agent action signatures with named payload parameter validation, including typed inline `image(... target=..., action/actions=...)` presentation actions, `capture(viewport|layer|object, ...) -> CaptureRef`, `attach(CaptureRef)`, `checkpoint(String)`, `note(DisplayText)`, and `rag.query(String, roots=..., graph_depth=..., limit=...) -> RagContextPack`.
- `arcweft-compiler` exposes `compile_agent_source_with_project`, which resolves `.awfagent` entity references against the module plus `ProjectSemanticIndex` and returns a `TypeCheckReport`.
- `arcweft-runtime-plan` can lower a typed single Agent controller body into the shared runtime-plan shape, and `arcweft-compiler` exposes `compile_agent_bundle_with_project` to produce an Agent controller `.awfb` data object with bytecode, embedded source, and `AgentArtifactManifest`.
- `arcweft-agent-protocol` owns Agent controller contract modules for artifact manifests, stable IDs, predicates, host requests/responses, traces, and typed values. Large host request/response enum payloads are boxed so the Rust protocol API does not carry oversized enum variants while preserving the same serde JSON shape.
- `arcweft-debug-model` provides Sans-I/O debug events, chunks, embedding records, RAG query models, and debug sink boundaries.
- `arcweft-rag` provides deterministic exact vector ranking and reciprocal-rank fusion primitives.
- `arcweft-agent-runner` provides the `AgentSession` host boundary, deterministic runtime policy checks, bounded wait polling with stable-frame confirmation, debug event emission, and a RAG service boundary for controller host calls. It can now run Agent controller bytecode through the shared core bytecode VM, dispatch effect-form Agent calls such as `observe(...)`, `checkpoint(...)`, `advance_text()`, `choose(...)`, `invoke(...)`, `capture(...)`, `read_resource(...)`, and `rag.query(...)`, evaluate structured wait predicates including `Compare`, `Exists`, `All`, `Any`, and `Not` against signal, metric, state-payload, and observation-field probes, and bridge Agent `HostTaskRequest::Custom` suspend/resume tasks back into the VM so expressions such as `let shot = capture(...)` can bind the returned capture record and continue execution.
- `arcweft-bundle` can now encode `.awfb` bundles as `bundle_kind = agent_controller` with an embedded `AgentArtifactManifest`, while ordinary game bundles remain the default. The game `arcweft-runtime-host` bundle runner rejects Agent controller bundles instead of executing them as game bytecode.
- `arcweft-debug-sqlite` provides the rebuildable `SQLite`/FTS5 debug index, event sink adapter, Japanese lexical smoke coverage, and little-endian f32 vector blob storage without unsafe casts.
- `arcw agent script check <file.awfagent>` validates Agent dialect parsing/HIR lowering without requiring the `native-capture` feature.
- `arcw agent script build <file.awfagent> --output <file.awfb>` compiles a single Agent controller through the typed project-index path and writes the shared `.awfb` bundle JSON with `bundle_kind = agent_controller` and the embedded `AgentArtifactManifest`. It does not create a separate Agent-only artifact container.
- `arcw agent script run <file.awfagent|file.awfb>` executes one Agent controller through `arcweft-agent-runner`. By default it uses the deterministic CLI session adapter; with `--native-source <file.arcw>` or `--profile <id>` under the `native-capture` feature, it uses the native game observation/capture/resource path as an `AgentSession`. Source input is compiled to the shared Agent `.awfb` bundle shape first; bundle input is decoded through `ArcweftBundle` and must carry `bundle_kind = agent_controller`. When source input is run against a native source/profile, the Agent compiler builds its `ProjectSemanticIndex` from the checked native HIR, so `.awfagent` references such as `@flow.opening` and `@signal.current_flow` resolve against the real project declarations instead of a CLI-only stub. CLI `--signal id=value` entries fill missing signal symbols for non-native sessions, but they do not overwrite typed signal declarations already present in the native project index. CLI `--state path=value` entries populate deterministic debug state payload values for `state("path")` probes in non-native sessions. CLI/native `--blob-dir <dir>` writes capture bytes under `blake3/<hex>` content-addressed paths and reports the stored byte count. Native observation signals are normalized from `@signal.id`/`@flow.id` surface spelling to Agent protocol keys and `Entity` values. Native Agent Script sessions keep the native runtime executor alive across observe/action/wait/capture/resource host calls. `choose(@choice...)` now dispatches as a semantic `SelectChoice` runtime input rather than a physical click fallback, and subsequent waits observe the advanced native runtime state. `advance_text()` now lowers to `AgentAction::AdvanceText`; the native session converts it to an `advance` runtime input only when the latest observation exposes an enabled semantic `AdvanceText` target. `invoke(target, action, args?)` now lowers to `AgentAction::Invoke`; the native session requires a matching enabled semantic `Invoke` action target from the latest observation and dispatches an `invoke` runtime input with the action id, preserving semantic action dispatch without falling back to coordinates. `--trace-out <file.arcwx>` writes a JSON `AgentTraceRecord` stream derived from the runner debug events, with deterministic `blake3:` payload hashes, explicit run/session IDs, and capture `blob_refs` copied from `CaptureResult.content_hash`.
- `arcw agent script trace <file.arcwx>` reads a trace through the shared `AgentTraceRecord` protocol type and validates the `.arcwx` extension, JSON shape, schema version, payload hash, run-id consistency, first/last record kinds, strictly increasing sequence numbers, and capture-record blob refs. With `--blob-dir <dir>`, it also validates byte-backed capture blobs by resolving each capture `content_hash` to `blake3/<hex>`, recomputing the stored bytes' hash, and comparing the stored byte length with the trace payload. It is a read-only validation surface for future replay/MCP trace resources, not a second runner.
- `arcw agent script replay <file.arcwx> [--expect <file.arcwx>]` performs read-only logical replay of validated trace records. The replay sequence records each event's sequence number, kind, tick, payload hash, and blob refs. `--expect` compares two traces by replay-relevant logical fields rather than byte-for-byte file identity, so it can verify that source-run and bundle-run traces produce the same logical sequence.
- `arcweft-agent-protocol` and `arcweft-agent-mcp` now expose Agent trace resources as typed MCP-compatible resources. `AgentResourceKind::Trace` plus `arcweft_agent_mcp::trace_resource` map validated `AgentTraceRecord` arrays to `arcweft://run/{run_id}/trace.arcwx` with `application/vnd.arcweft.agent-trace+json`, and `resources/templates/list` includes the trace URI pattern.
- `arcw agent mcp` exposes `arcweft.trace.read` in the stdio MCP transport. It validates a `.arcwx` file through the same trace reader used by CLI replay, caches the resulting trace resource in the current MCP session, and serves it through `resources/list`, `resources/read`, and `arcweft.resource.read` without requiring a prior native observation.
- `arcw agent mcp` exposes `arcweft.action` for enabled semantic Agent actions from the latest native observation, or after observing a supplied `source`/`profile` first. It keeps the native runtime executor alive across observe/action calls, dispatches `AdvanceText`, `SelectChoice`, and `Invoke` through semantic runtime inputs, accepts optional JSON object `args` for direct invoke calls, and returns accepted before/after ticks and state hashes plus the post-action frame summary. It does not synthesize physical pointer actions.
- `arcw agent mcp` exposes read-only debug tools `arcweft.get_state`, `arcweft.signal_get`, and `arcweft.log_query`. They read the latest cached Agent observation, or run `arcweft.observe` first when `source` or `profile` arguments are supplied, and return structured JSON text blocks without mutating runtime state.
- `arcw agent mcp` exposes read-only `arcweft.rag.query`. It builds an explainable `RagContextPack` from cached observation state/actions/objects/signals/metrics/logs/diagnostics and cached `.arcwx` trace records, can observe a supplied `source`/`profile` first, ranks candidates with deterministic reciprocal-rank fusion from `arcweft-rag`, and preserves `roots`, `graph_depth`, `limit`, `max_context_bytes`, `max_privacy`, channels, chunk IDs, and context item bodies in the returned JSON. RAG candidates above `max_privacy` are filtered before ranking; observation-derived candidates default to `project`, and trace records can override that with `privacy_class` or `privacy` fields at the record or nested payload level.
- `arcw agent repl <file.arcw|--profile <id>> --input <session.txt> --json` is the first deterministic Agent REPL frontend slice. It reads scripted cells from a file or stdin, keeps meta commands under the `:` namespace, uses the shared Agent fragment parser to classify non-meta cells, and delegates `:observe` / `:actions` to the native Agent observation/action-target surface. It deliberately does not evaluate ordinary DSL cells yet, avoiding a second evaluator before the compiler-backed transactional cell runner lands.
- `arcw agent rag query --trace <file.arcwx> --query <text>` exposes the same `RagContextPack` JSON shape for standalone CLI use. It validates the trace through the shared `.arcwx` reader, chunks the trace summary and individual `AgentTraceRecord` payloads, applies deterministic reciprocal-rank fusion from `arcweft-rag`, and supports `--root`, `--graph-depth`, `--limit`, `--max-context-bytes`, `--max-privacy`, and `--json`. The default `--max-privacy project` excludes `sensitive` and `secret` chunks; `--max-privacy public` only returns records explicitly marked public.
- `arcw debug db status|migrate` opens and migrates the rebuildable Agent debug `SQLite` database at `.arcweft/cache/agent-debug.sqlite3` by default and reports row counts for core Agent debug tables.
- `arcw debug db validate` runs SQLite integrity checks, foreign-key checks, capture-to-blob reference validation, and embedding vector blob length validation. With `--blob-dir <dir>`, it also validates indexed blob `relative_path` values against byte-backed files, rejecting unsafe paths, missing files, and byte-length mismatches. `arcw debug db reindex` rebuilds and optimizes the derived chunk FTS index. `arcw debug db search --query <text> [--max-privacy public|project|sensitive|secret]` exposes the rebuildable debug index's literal FTS5 lexical search with privacy filtering inside the SQL query before `--limit` is applied. `arcw debug db delete --unreferenced-blobs [--blob-dir <dir>] [--validate]` deletes unreferenced blob records from the debug-store index and, when a blob directory is supplied, deletes the corresponding safe unreferenced files while preserving capture-referenced files.
- `samples/agent-script/opening-smoke.awfagent` and `samples/agent-script/visual-regression.awfagent` mirror the package examples and currently pass `agent script check`. `samples/agent-script/cli-run-smoke.awfagent` is the minimal deterministic CLI runner smoke, `samples/agent-script/cli-advance-text-smoke.awfagent` covers `advance_text()` intrinsic dispatch through the deterministic CLI session, `samples/agent-script/cli-capture-smoke.awfagent` covers deterministic capture trace blob refs and byte-backed blob validation without depending on native game state, `samples/agent-script/cli-composite-wait-smoke.awfagent` covers CLI signal-backed composite wait predicates, `samples/agent-script/cli-state-wait-smoke.awfagent` covers CLI state-payload and observation-field wait predicates, `samples/agent-script/native-flow-wait-smoke.awfagent` plus `samples/agent-script/native-project-index.arcw` cover native HIR project-index entity resolution and entity-valued signal wait polling, `samples/agent-script/native-choice-dispatch.awfagent` plus `samples/agent-script/native-choice-dispatch.arcw` cover native semantic `SelectChoice` dispatch followed by signal wait validation, `samples/agent-script/native-advance-text-game.awfagent` covers enabled-observation-gated `AdvanceText` dispatch in native Game step mode, and `samples/agent-script/native-invoke-action.awfagent` covers native semantic `Invoke` dispatch against a projected image action from `samples/image-animation.arcw`.

## Deliberate boundaries

- Agent Script reuses the existing expression, statement, block, attribute, contract/effect, and HIR lowering path.
- No compatibility shim for the old line-command syntax is present.
- `compile_agent_source` remains the parser/HIR-only entry point for lightweight syntax checks; `compile_agent_source_with_project` is the typed project-index entry point; `compile_agent_bundle_with_project` is the typed artifact entry point. Controller VM execution and host-call dispatch are owned by `arcweft-agent-runner`; the CLI native game adapter path currently supports observe, wait polling, capture, resource read, semantic `SelectChoice`, enabled-observation-gated `AdvanceText` including native Game-mode E2E validation, semantic `Invoke`, privacy-filtered standalone trace-backed CLI RAG query, privacy-filtered standalone debug DB lexical search, deterministic REPL observe/actions meta commands, and MCP observe/action/capture/resource/state/signal/log/trace/RAG tools through the existing native Agent resource/runtime builders. Compiler-backed transactional REPL cell execution, broader non-lexical standalone CLI debug search, and MCP control surfaces beyond semantic action dispatch remain follow-up work.
- Agent source defaults such as omitted signature return type are preserved as syntax/HIR shape for later semantic/compiler resolution rather than being string-rewritten in the parser.
- `invoke` is checked as a semantic action contract on a typed project entity. It does not accept string target IDs, does not fall back to physical actions, and validates payload record keys and values against named project-index Agent action parameters.
- `arcweft-agent-contract-reference` was not added as a production crate. Its concepts were merged into `arcweft-agent-protocol` modules to avoid duplicate protocol surfaces.
- `arcweft-debug-model` and `arcweft-rag` are Sans-I/O crates. They do not open databases, read files, call embedding services, or inspect runtime state directly.
- `arcweft-agent-runner` executes typed host requests and can step Agent controller bytecode for both effect-form calls and suspended Agent host-call expressions. It bridges `agent` custom host tasks to the same `AgentHostRequest` boundary and resumes the VM with a typed record payload. `wait(...)` expression tasks and Agent statement-form `wait(predicate, timeout=...)` lower typed probe comparisons and boolean predicate combinators to structured predicate records and execute through the same host wait boundary as `capture(...)`.
- Parser dialect dispatch keeps Agent `wait(...)` as an intrinsic call expression even when it appears as a bare statement. Game dialect line-task `wait(...)` remains the legacy dialogue/runtime wait statement path.
- `arcweft-debug-sqlite` is the only new I/O crate in this slice. It owns `rusqlite`, lifecycle validation, derived index rebuild, and debug-store record deletion while keeping database access out of syntax, HIR, compiler, runner, protocol, debug-model, and RAG crates.

## Windows validation

- `cargo check -p arcweft-debug-model -p arcweft-rag -p arcweft-debug-sqlite`
- `cargo test -p arcweft-debug-model -p arcweft-rag -p arcweft-debug-sqlite`
- `cargo test -p arcweft-debug-sqlite`
- `cargo clippy -p arcweft-debug-model -p arcweft-rag -p arcweft-debug-sqlite --all-targets --all-features -- -D warnings`
- The `arcweft-debug-sqlite` tests were run on Windows and validate migration, FTS5 Japanese search, and embedding blob round trips.
- `cargo fmt --check`
- `cargo check -p arcweft-cli -p arcweft-lang-syntax`
- `cargo check -p arcweft-cli --features native-capture`
- `cargo test -p arcweft-lang-syntax agent_dialect -- --nocapture`
- `cargo clippy -p arcweft-cli -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-lang-sema -p arcweft-compiler`
- `cargo test -p arcweft-lang-sema project_index -- --nocapture`
- `cargo test -p arcweft-lang-sema project_index_from_hir_preserves_flow_and_signal_ref_value_types -- --nocapture`
- `cargo test -p arcweft-lang-sema typecheck -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_source_with_project -- --nocapture`
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_body_to_entry_flow -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_bundle -- --nocapture`
- `cargo check -p arcweft-runtime-plan -p arcweft-compiler`
- `cargo test -p arcweft-compiler -- --nocapture`
- `cargo clippy -p arcweft-agent-protocol --all-targets --all-features -- -D warnings`
- `cargo test -p arcweft-agent-protocol -p arcweft-agent-mcp`
- `cargo clippy -p arcweft-agent-protocol -p arcweft-agent-mcp --all-targets --all-features -- -D warnings`
- `cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-agent-protocol -p arcweft-agent-runner`
- `cargo test -p arcweft-agent-protocol -p arcweft-agent-runner`
- `cargo test -p arcweft-agent-protocol -p arcweft-runtime-plan -p arcweft-agent-runner`
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_host_call_let_to_await -- --nocapture`
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_wait_predicate_to_host_task -- --nocapture`
- `cargo test -p arcweft-agent-runner controller_bytecode_dispatches_effect_calls_to_runner_host_boundary -- --nocapture`
- `cargo test -p arcweft-agent-runner effect_form_invoke_call_lowers_to_host_action -- --nocapture`
- `cargo test -p arcweft-agent-runner controller_bytecode_resumes_bound_capture_response -- --nocapture`
- `cargo test -p arcweft-agent-runner controller_bytecode_resumes_bound_wait_response -- --nocapture`
- `cargo test -p arcweft-agent-runner wait_matches_entity_probe_against_string_observation_id -- --nocapture`
- `cargo test -p arcweft-agent-runner effect_form_wait_call_lowers_to_host_wait_request -- --nocapture`
- `cargo test -p arcweft-agent-runner effect_form_advance_text_call_lowers_to_host_action -- --nocapture`
- `cargo test -p arcweft-agent-runner effect_form_wait_call_lowers_composite_predicate -- --nocapture`
- `cargo test -p arcweft-agent-runner wait_matches_composite_float_predicate -- --nocapture`
- `cargo test -p arcweft-agent-runner wait_matches_state_and_observation_field_predicates -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_source_with_project_checks_statement_wait_entity_probe -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_source_with_project_checks_composite_predicates -- --nocapture`
- `cargo test -p arcweft-compiler compile_agent_source_with_project_checks_state_and_observation_probes -- --nocapture`
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_composite_wait_predicates_to_host_task -- --nocapture`
- `cargo test -p arcweft-runtime-plan agent_controller_plan_lowers_state_and_observation_wait_predicates -- --nocapture`
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
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/cli-run-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/cli-capture-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/cli-advance-text-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/native-advance-text-game.awfagent --json`
- `cargo run -p arcweft-cli -- agent script check samples/agent-script/native-invoke-action.awfagent --json`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-advance-text-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script build samples/agent-script/cli-run-smoke.awfagent --output target/codex-agent-script-final/cli-run-smoke.awfb --json`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-run-smoke.awfagent --json`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-composite-wait-smoke.awfagent --signal signal.ready=true --json --trace-out target/codex-agent-script-final/cli-composite-wait-smoke.arcwx`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-state-wait-smoke.awfagent --state route.phase=opening --json --trace-out target/codex-agent-script-final/cli-state-wait-smoke.arcwx`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-capture-smoke.awfagent --json --trace-out target/codex-agent-script-final/cli-capture-smoke.arcwx`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-capture-smoke.awfagent --json --trace-out target/codex-agent-script-final/cli-capture-blob-smoke.arcwx --blob-dir target/codex-agent-script-final/agent-blobs`
- `cargo run -p arcweft-cli -- agent script trace target/codex-agent-script-final/cli-capture-blob-smoke.arcwx --blob-dir target/codex-agent-script-final/agent-blobs --json`
- `cargo run -p arcweft-cli --features native-capture -- agent script run samples/agent-script/cli-capture-smoke.awfagent --native-source samples/rich-text-showcase.arcw --json --trace-out target/codex-agent-script-final/native-cli-capture-smoke.arcwx --blob-dir target/codex-agent-script-final/native-agent-blobs`
- `cargo run -p arcweft-cli --features native-capture -- agent script trace target/codex-agent-script-final/native-cli-capture-smoke.arcwx --blob-dir target/codex-agent-script-final/native-agent-blobs --json`
- `cargo run -p arcweft-cli --features native-capture -- agent script run samples/agent-script/native-flow-wait-smoke.awfagent --native-source samples/agent-script/native-project-index.arcw --json --trace-out target/codex-agent-script-final/native-flow-wait-smoke.arcwx`
- `cargo run -p arcweft-cli --features native-capture -- agent script run samples/agent-script/native-choice-dispatch.awfagent --native-source samples/agent-script/native-choice-dispatch.arcw --json --trace-out target/codex-agent-script-final/native-choice-dispatch.arcwx`
- `cargo run -p arcweft-cli --features native-capture -- agent script run samples/agent-script/native-advance-text-game.awfagent --native-source samples/rich-text-showcase.arcw --native-mode game --native-steps 1 --json --trace-out target/codex-agent-script-final/native-advance-text-game.arcwx`
- `cargo run -p arcweft-cli --features native-capture -- agent script run samples/agent-script/native-invoke-action.awfagent --native-source samples/image-animation.arcw --flow image_clipped_object --json --trace-out target/codex-agent-script-final/native-invoke-action.arcwx`
- `cargo run -p arcweft-cli -- agent script run samples/agent-script/cli-run-smoke.awfagent --json --trace-out target/codex-agent-script-final/cli-run-smoke.arcwx`
- `cargo run -p arcweft-cli -- agent script run target/codex-agent-script-final/cli-run-smoke.awfb --json --trace-out target/codex-agent-script-final/cli-run-smoke-bundle.arcwx`
- `cargo run -p arcweft-cli -- agent script trace target/codex-agent-script-final/cli-run-smoke.arcwx --json`
- `cargo run -p arcweft-cli -- agent script trace target/codex-agent-script-final/cli-run-smoke-bundle.arcwx --json`
- `cargo run -p arcweft-cli -- agent script replay target/codex-agent-script-final/cli-run-smoke.arcwx --expect target/codex-agent-script-final/cli-run-smoke-bundle.arcwx --json`
- `cargo test -p arcweft-cli --test check agent_script_run_trace_records_capture_blob_refs -- --exact --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_script_run_native_source_captures_native_resource -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_script_run_native_source_resolves_project_entities -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_script_run_native_source_dispatches_semantic_choice_action -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_script_run_native_source_dispatches_advance_text_in_game_mode -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_script_run_native_source_dispatches_semantic_invoke_action -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture native_agent_invoke_requires_enabled_semantic_action -- --nocapture`
- `cargo clippy -p arcweft-agent-runner -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio_reads_agent_trace_resource -- --exact --ignored --nocapture`
- `cargo test -p arcweft-agent-mcp debug_read_tool_schemas_expose_state_signal_and_log_filters -- --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio_dispatches_semantic_action -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture --test check agent_repl_observes_and_lists_actions_from_input_session -- --exact --ignored --nocapture`
- `cargo test -p arcweft-cli --features native-capture agent_mcp_debug_read_tools_return_cached_observation_data -- --nocapture`
- `cargo test -p arcweft-cli --features native-capture agent_mcp_rag_query_returns_explainable_context_pack -- --nocapture`
- `cargo clippy -p arcweft-agent-mcp -p arcweft-cli --features native-capture --all-targets -- -D warnings`
- `target\debug\arcw.exe agent mcp` with JSON-RPC `initialize` and `tools/list`, confirming `arcweft.rag.query`.
- `cargo test -p arcweft-cli --test check agent_script_run_json_executes_cli_session_smoke -- --exact --nocapture`, including `arcw agent rag query --trace <generated.arcwx> --query observation_received --root observation_received --json`.
- `cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli -- agent rag query --trace target\codex-agent-rag-cli-smoke\cli-run-smoke.arcwx --query observation_received --root observation_received --json`
- `cargo test -p arcweft-cli --test check agent_script_run_json_executes_cli_session_smoke -- --exact --nocapture`
- `cargo test -p arcweft-cli --test check agent_script_run_json_executes_advance_text_smoke -- --exact --nocapture`
- `cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli -- debug db status --path target/codex-agent-script-final/agent-debug-test.sqlite3 --json`
- `cargo run -p arcweft-cli -- debug db status --path target\codex-agent-script-final\agent-debug-lifecycle.sqlite3 --json`
- `cargo run -p arcweft-cli -- debug db validate --path target\codex-agent-script-final\agent-debug-lifecycle.sqlite3 --json`
- `cargo run -p arcweft-cli -- debug db reindex --path target\codex-agent-script-final\agent-debug-lifecycle.sqlite3 --json`
- `cargo test -p arcweft-debug-sqlite lexical_search_filters_by_max_privacy_before_limit -- --nocapture`
- `cargo test -p arcweft-cli --test check debug_db_search_filters_chunks_by_privacy -- --exact --nocapture`
- `cargo run -p arcweft-cli -- debug db delete --path target\codex-agent-script-final\agent-debug-lifecycle.sqlite3 --unreferenced-blobs --validate --json`
- `cargo test -p arcweft-debug-sqlite delete_unreferenced_blobs_keeps_referenced_capture_blobs -- --nocapture`
- `cargo test -p arcweft-cli delete_blob_files_removes_only_safe_existing_files -- --nocapture`
- `cargo test -p arcweft-cli checked_blob_file_path_rejects_paths_outside_blob_dir -- --nocapture`
- `cargo clippy -p arcweft-debug-sqlite -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli -- debug db validate --path target\codex-agent-script-final\agent-debug-blob-lifecycle.sqlite3 --blob-dir target\codex-agent-script-final\agent-debug-blob-files --json`
- `cargo run -p arcweft-cli -- debug db delete --path target\codex-agent-script-final\debug-db-blob-delete-smoke\agent-debug.sqlite3 --blob-dir target\codex-agent-script-final\debug-db-blob-delete-smoke\blobs --unreferenced-blobs --validate --json`
- `cargo check -p arcweft-debug-model -p arcweft-agent-mcp -p arcweft-cli --features native-capture`
- `cargo test -p arcweft-agent-mcp debug_read_tool_schemas_expose_state_signal_and_log_filters -- --nocapture`
- `cargo test -p arcweft-cli --features native-capture agent_mcp_rag_query_enforces_max_privacy -- --nocapture`
- `cargo test -p arcweft-cli --test check agent_script_run_json_executes_cli_session_smoke -- --exact --nocapture`
- `cargo clippy -p arcweft-debug-model -p arcweft-agent-mcp -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli -- agent rag query --trace target\codex-agent-rag-privacy-smoke\cli-run-smoke.arcwx --query observation_received --max-privacy public --json`

## Other platforms

- Linux/macOS should run the same check/test/clippy commands above.
- No Linux/macOS runtime validation has been performed in this workspace yet.

## Remaining zip-derived work

- Type Agent references against actions and resources beyond the current choice/layer/signal/metric plus typed inline image-action project-index coverage.
- Extend Agent predicate lowering beyond the currently executable signal/metric/state-payload/observation-field compare, exists, all, any, and not path as the Agent Prelude grows typed debug path registries and additional predicate surfaces.
- Extend CLI script run from native observe/wait/capture/resource/semantic `SelectChoice`/`Invoke` plus enabled-observation-gated `AdvanceText` sessions to compiler-backed transactional REPL cell execution, vector/graph/history standalone CLI debug search beyond current trace-backed RAG and DB lexical search, and MCP control surfaces beyond the current observe/action/capture/resource/state/signal/log/trace/RAG tools.
- Extend privacy classification enforcement beyond current CLI/MCP RAG context filtering into product-mode resource redaction, audit logging, persistence policy, and non-RAG debug reads.
- Continue Windows end-to-end validation as REPL and MCP action/control surfaces are implemented.
