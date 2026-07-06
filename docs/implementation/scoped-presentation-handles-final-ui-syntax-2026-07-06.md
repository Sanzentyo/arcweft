# Scoped Presentation Handles And Final UI Syntax - 2026-07-06

This cut implements the first production slice of scoped presentation resource
handles and final component/View authoring syntax.

## Implemented

- Added runtime `FlowOp::RegisterCleanup` and `FlowOp::CancelCleanup` support.
  The structured engine records cleanup effects on the active lexical scope,
  drains them in LIFO order on scope exit, drains root cleanups on flow return or
  goto, and allows explicit cancellation for manual release/dispose paths.
- Added AWBC instruction, codec, verifier, VM, and product parity support for
  cleanup registration and cancellation.
- Lowered value-position `let panel = component(...)` and `let image =
  image(...)` calls to `presentation.handle.create`, scoped cleanup
  registration, and a stable string handle binding.
- Lowered handle methods `show`, `hide`, `unmount`, `release`, `destroy`,
  `close`, and `dispose` to `presentation.handle.*` effects. Terminal methods
  cancel the registered cleanup.
- Split presentation-handle helper logic into
  `crates/arcweft-runtime-plan/src/flow/presentation.rs` so the main flow
  lowerer stays below the structure-audit error threshold.
- Changed current component/View authoring syntax to `component Name() { ... }`
  with canonical `Panel`, `Column`, `Row`, and `Stack` elements. Removed
  `component ... -> View` and `Surface` / `VStack` / `HStack` as accepted
  component body syntax; they now produce structured parse diagnostics.
- Added canonical `Box` and `Scroll` View element vocabulary to parser,
  bundle-side `UiElementKind`, style selectors, and component/View sidecar
  lowering. `Box` lowers as a stack-style container and `Scroll` lowers as a
  typed vertical container so authored resource contracts no longer collapse to
  custom elements.
- Added presentation handle table epochs to runtime display snapshots. Create,
  live-state, and terminal transitions now advance a deterministic operation
  epoch, serialize created/updated epochs, preserve tombstones through serde
  roundtrips, and reject stale operations after rollback.
- Added AWBC fiber checkpoint coverage for root and lexical cleanup stacks so
  cleanup registrations survive serde save/load-style restoration in the core
  fiber state.
- Added the first typed action declaration substrate: `action` is now a
  canonical entity declaration family, parses as `EntityDeclKind::Action`,
  lowers through HIR declarations, registers as `EntityKind::Action`, and
  resolves `@action...` references. Payload signature checking and event
  dispatch are intentionally left to the follow-up action/receive slice.
- Updated current samples, parser fixtures, and stable docs/examples to use the
  canonical syntax. Historical review request markdown remains unchanged.

## Verification

- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test -p arcweft-core --all-features cleanup`
- `cargo test -p arcweft-runtime-plan --all-features value_position_component_handle_lowers_to_create_cleanup_and_close_cancel`
- `cargo test -p arcweft-runtime-plan --all-features awbc_product_parity_scope_cleanup_and_cancel`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-syntax --all-features component_view_box_and_scroll_parse_as_canonical_elements`
- `cargo test -p arcweft-lang-sema --all-features`
- `cargo test -p arcweft-core --all-features fiber_checkpoint_and_serde_preserve_cleanup_stacks`
- `cargo test -p arcweft-runtime-driver --all-features presentation`
- `cargo test -p arcweft-cli --all-features component_view_box_and_scroll_lower_to_typed_ui_resources`
- `cargo test -p arcweft-lang-syntax --all-features action_declaration_parses_as_typed_entity`
- `cargo test -p arcweft-lang-sema --all-features action_entity`
- `cargo test -p arcweft-lang-sema --all-features parses_entity_declarations_used_by_presentation_docs`
- `cargo test -p arcweft-cli --all-features --test native_text_input_sample_sidecars`
- `cargo test -p arcweft-cli --all-features --test native_text_input_native_interactive_smoke`
- `cargo test -p arcweft-cli --all-features --test css_style_parity_sample`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The final structure audit reported 0 errors and 138 warnings. Relevant current
file sizes:

- `crates/arcweft-runtime-plan/src/flow.rs`: 2413 physical LOC, production,
  no embedded tests.
- `crates/arcweft-runtime-plan/src/flow/presentation.rs`: 105 physical LOC,
  production, no embedded tests.
- `crates/arcweft-core/src/engine/flow.rs`: 988 physical LOC, production, no
  embedded tests.
- `crates/arcweft-core/src/awbc/vm.rs`: 1461 physical LOC, production, no
  embedded tests; existing size warning remains.
- `crates/arcweft-cli/src/app/bundle.rs`: 2157 physical LOC, production, no
  embedded tests; existing size warning remains.
- `crates/arcweft-lang-syntax/src/parser/items.rs`: 1354 physical LOC,
  production, no embedded tests; existing size warning remains.
- `crates/arcweft-lang-syntax/src/parser/view.rs`: 967 physical LOC,
  production, no embedded tests.

## Action Invoke Button Substrate

- Component/View `Button(...).on_click { action.invoke(@action:.name, value = expr) }`
  now parses into a typed `ViewAction::ActionInvoke` activation. The parser
  accepts the block callback form and normalizes both call-shaped
  `action.invoke(...)` and method-call-shaped `action.invoke(...)` expression
  ASTs into the same action node.
- `UiActionButtonActionResource`, `UiRuntimeActionButtonAction`, and
  `RenderActionButtonAction` now carry `ActionInvoke { action, payload }`
  alongside the existing `TextInputSubmit` route. Runtime action-button
  lowering validates the authored action public id before rendering.
- Rendered action buttons register their authored action id in the semantic
  tree, and player-scene pointer/keyboard activation lowers it into
  `InputOutcome.actions`. Native and web session bridges now accept generic
  semantic actions by queueing a deterministic `action.invoke` custom input
  targeted at the action id instead of rejecting anything except
  `action.choice.select`.
- Payloads are currently stored as authored expression source strings. Literal
  string payloads are stored as literal values, while values such as
  `visitor_name.text` are preserved as source text until the typed input-handle
  evaluator and payload expression evaluator are implemented.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features component_view_button_on_click_action_invoke_block_parses`
- `cargo test -p arcweft-bundle --all-features runtime_action_button_resolves_action_invoke_action`
- `cargo test -p arcweft-player-scene --all-features pointer_activation_on_action_invoke_button_emits_semantic_action`
- `cargo test -p arcweft-runtime-driver --all-features session_accepts_generic_semantic_action_invoke`
- `cargo test -p arcweft-cli --all-features component_view_action_invoke_button_lowers_to_action_resource`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The action-invoke cut was measured at Jujutsu change `nqnzzvoz` /
`39e9c9c5`. The current structure audit still reports 0 errors and 138
warnings. Relevant changed production files:

| Path | Bytes | LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 35,170 | 967 | production | View element/modifier parsing and action callback normalization |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 19,257 | 726 | production | Typed component/View AST, including button activation payloads |
| `crates/arcweft-bundle/src/resource_codec/ui/model.rs` | 49,503 | 1,480 | production with embedded tests | UI resource/runtime model and runtime projection |
| `crates/arcweft-bundle/src/resource_codec/ui/codec.rs` | 39,725 | 1,080 | production | UI resource codec reference accounting |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 46,123 | 1,227 | production | Component/View sidecar lowering into bundle resources |
| `crates/arcweft-player-scene/src/action_buttons.rs` | 5,116 | 130 | production | Runtime action-button resource lowering |
| `crates/arcweft-player-scene/src/input.rs` | 46,658 | 1,245 | production with embedded tests | Routed input, focus, text editing, and action-button activation |
| `crates/arcweft-render-wgpu/src/geometry/action_buttons.rs` | 8,460 | 227 | production | Action-button render geometry and semantic node emission |
| `crates/arcweft-runtime-driver/src/session.rs` | 58,208 | 1,433 | production with embedded tests | Bundle session input queueing and runtime bridge |

Relevant changed test files:

| Path | Bytes | LOC |
| --- | ---: | ---: |
| `crates/arcweft-bundle/tests/ui_action_button_resources.rs` | 3,969 | 100 |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 26,667 | 790 |
| `crates/arcweft-lang-syntax/tests/style_component_view.rs` | 9,207 | 346 |
| `crates/arcweft-player-scene/tests/action_button_submit.rs` | 7,493 | 183 |
| `crates/arcweft-runtime-driver/tests/session.rs` | 35,874 | 905 |

## Receive Action Flow Primitive

- Added structured flow syntax for `let event = receive action(@action:.name)`.
  The parser records this as `Stmt::LetActionReceive` rather than a generic call
  expression so runtime-plan lowering can preserve the suspension contract.
- Type checking now requires the receive target to be `Ref<Action>` and binds
  the result as the nominal `ActionEvent` type. `ActionEvent.action` projects as
  `Ref<Action>` and `ActionEvent.value` projects as `String`, matching the
  current runtime payload representation.
- Runtime-plan lowering emits a suspending `ui.action.await` host call with the
  action target as a typed argument. The runtime driver captures those host
  calls, keeps pending action receives by action id, and resumes the fiber with
  a record payload when a queued semantic action with the matching id arrives.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features flow_receive_action_statement_is_structured`
- `cargo test -p arcweft-lang-sema --all-features typechecks_receive_action_event_value_projection`
- `cargo test -p arcweft-runtime-plan --all-features receive_action_lowers_to_ui_action_host_call`
- `cargo test -p arcweft-runtime-driver --all-features session_receive_action_host_call_resumes_with_event_value`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The receive-action cut was measured at Jujutsu change `mrqpuknq`. The structure
audit reported 0 errors and 138 warnings. Current changed Rust file metrics:

| Path | Bytes | LOC | Classification | Embedded Tests |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-agent-repl/src/binding.rs` | 11979 | 371 | production | false |
| `crates/arcweft-cli/src/app/bundle/component_mounts.rs` | 16785 | 442 | production | false |
| `crates/arcweft-lang-sema/src/checker.rs` | 29350 | 831 | production | false |
| `crates/arcweft-lang-sema/src/checker/stmt.rs` | 23075 | 598 | production | false |
| `crates/arcweft-lang-sema/src/project_index.rs` | 30873 | 1096 | production | false |
| `crates/arcweft-lang-sema/src/project_index/entities.rs` | 32897 | 915 | production | false |
| `crates/arcweft-lang-sema/src/project_index/flow_control.rs` | 16579 | 484 | production | false |
| `crates/arcweft-lang-sema/src/project_index/relations.rs` | 42970 | 1186 | production | false |
| `crates/arcweft-lang-sema/src/semantic.rs` | 76670 | 2054 | production | false |
| `crates/arcweft-lang-sema/src/semantic/traversal.rs` | 30064 | 831 | production | false |
| `crates/arcweft-lang-sema/src/symbols.rs` | 36623 | 1087 | production | false |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 65821 | 2202 | test | false |
| `crates/arcweft-lang-sema/src/types.rs` | 9572 | 384 | production | false |
| `crates/arcweft-lang-syntax/src/ast/flow.rs` | 23407 | 1020 | production | false |
| `crates/arcweft-lang-syntax/src/parser/statements.rs` | 18397 | 546 | production | false |
| `crates/arcweft-lang-syntax/tests/parser_p1.rs` | 12346 | 431 | test | false |
| `crates/arcweft-lsp/src/features/actions.rs` | 53403 | 1646 | production | true |
| `crates/arcweft-lsp/src/features/cascade.rs` | 32171 | 888 | production | false |
| `crates/arcweft-runtime-driver/src/session.rs` | 60885 | 1610 | production | false |
| `crates/arcweft-runtime-driver/tests/session.rs` | 39338 | 1097 | test | false |
| `crates/arcweft-runtime-plan/src/flow.rs` | 89732 | 2442 | production | false |
| `crates/arcweft-runtime-plan/tests/runtime_plan.rs` | 49783 | 1618 | test | false |
| `crates/arcweft-tooling/src/dialogue_content.rs` | 8406 | 263 | production | false |
| `crates/arcweft-tooling/src/speaker_presets.rs` | 26758 | 684 | production | false |
| `crates/arcweft-verify/src/lib.rs` | 67054 | 1938 | production | false |

## Action Payload Input Handle Evaluation

- Runtime-driver action dispatch now resolves action payloads shaped as text
  control handle projections before queueing the deterministic `action.invoke`
  input event or resuming a pending `receive action(...)` host call.
- Supported projections are `.text` and `.value` on active runtime text-control
  IDs. Both canonical `input.visitor_name.text` and component-local shorthand
  `visitor_name.text` resolve to the matching active runtime text control. The
  `@input:.visitor_name.text` spelling is also accepted after normalization.
- Resolution is deliberately single-match only. If no active text control
  matches, or more than one active control would match the shorthand, the
  payload remains the authored source string rather than guessing the wrong
  control.
- The session-level receive-action regression now mounts a real
  `input.visitor_name` UI resource with value `Ada` and verifies that
  `action.invoke(..., value = visitor_name.text)` resumes `event.value` as
  `Ada`, not the authored source string.
- This is still a runtime bridge over the current `Option<String>` payload
  resource. A later resource/schema cut should replace the string payload with
  a typed action-payload expression enum so literal strings such as
  `"visitor_name.text"` cannot be confused with handle projections.

### Verification

- `cargo test -p arcweft-runtime-driver --all-features action_payload_value`
- `cargo test -p arcweft-runtime-driver --all-features session_receive_action_host_call_resumes_with_event_value`
- `cargo test -p arcweft-runtime-driver --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The action payload input-handle cut kept the structure audit at 0 errors and
138 warnings.

## Remaining Work

- End-to-end save subsystem wiring still needs to consume the runtime display
  snapshot and AWBC fiber cleanup checkpoint evidence added here. This cut
  verifies serde roundtrip and rollback substrate, not a full player save/load
  scenario.
- Component/image scoped capture still needs precise hidden, unmounted,
  released, and destroyed handle diagnostics and native/web/observe parity
  tests.
- Lexical cleanup integration for overlay pop and scene transition needs the
  owning overlay/scene lifecycle operations to call the cleanup drain path.
- `Scroll` is now a typed resource and sidecar element, but scroll offsets,
  clipping, input routing, save/restore of scroll state, and native/web/observe
  parity tests still need the dedicated scroll runtime behavior slice.
- The final UI syntax direction still needs action payload sema/evaluation
  contracts beyond the current string payload bridge, typed input-handle
  expression evaluation for values such as `visitor_name.text`, generic
  callback block sugar beyond the `on_click` `action.invoke` route, and richer
  reactive branching surface from the broader input/scroll syntax request.
