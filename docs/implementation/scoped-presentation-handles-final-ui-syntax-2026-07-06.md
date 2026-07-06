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
  resolves `@action...` references. Event dispatch is covered by the later
  action-invoke and receive-action slices below.
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
- Action payloads are typed at the syntax and UI resource boundary. Literal
  strings lower as `LiteralString`, while text-control projections such as
  `visitor_name.text` lower as `TextControlProjection` targeting the canonical
  `input.visitor_name` runtime text control.

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

## Typed Action Payload Resource

- Replaced the action-button UI resource payload string with
  `UiActionPayloadResource`. `LiteralString` now represents authored string
  literals, and `TextControlProjection { input, field }` represents `.text` or
  `.value` projections from runtime text-control handles.
- Component/View syntax records `ViewActionPayload` instead of raw source text.
  The parser accepts literal strings and text-control projections for
  `action.invoke(..., value = ...)`; unsupported expressions are not silently
  preserved as payload source strings.
- Component/View sidecar lowering normalizes shorthand projections such as
  `visitor_name.text` to canonical `input.visitor_name` resource references.
  The UI resource codec now includes the referenced input ID in the program
  section public-id table, so action payload dependencies are visible to
  tooling and patch compatibility.
- Runtime action-button lowering resolves typed text-control projections while
  it still has the current `RenderTextInputControl` snapshots. Player-scene
  activation therefore emits a final semantic `Action` payload such as `Ada`;
  the runtime-driver no longer guesses whether an arbitrary string payload is a
  handle expression.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features component_view_button_on_click_action_invoke_block_parses`
- `cargo test -p arcweft-bundle --all-features runtime_action_button_resolves_action_invoke_action`
- `cargo test -p arcweft-cli --all-features component_view_action_invoke_button_lowers_to_action_resource`
- `cargo test -p arcweft-player-scene --all-features runtime_action_invoke_payload_reads_text_control_projection`
- `cargo test -p arcweft-player-scene --all-features pointer_activation_on_action_invoke_button_emits_semantic_action`
- `cargo test -p arcweft-runtime-driver --all-features session_accepts_generic_semantic_action_invoke`
- `cargo test -p arcweft-runtime-driver --all-features session_receive_action_host_call_resumes_with_event_value`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The typed-payload cut kept the structure audit at 0 errors and 138 warnings.

## Component Scoped Capture And Handle Visibility

- UI resource metadata now carries the owning component id from Component/View
  lowering into `UiSemanticTarget`, `UiInputOptions`, `UiActionButtonResource`,
  `UiRuntimeTextControl`, and `UiRuntimeActionButton`. The field is optional so
  legacy/non-component resources keep the existing top-level behavior.
- Runtime presentation-handle filtering now treats a live component handle id as
  an alias for its owned runtime text controls and action buttons. Hiding,
  unmounting, releasing, or destroying a component handle removes those child
  controls from the presentation snapshot; showing the handle restores them.
- Agent native observe now preserves component ownership for runtime semantic
  objects by mapping prepared text-input and button targets back to their owning
  component ids. Component grouping therefore reports the authored component
  scope instead of falling back to each object id.
- Agent observe now emits structured `AGENT_CAPTURE_MISSING_SCOPE` diagnostics
  when a requested `--component`, `--object`, or `--layer` capture scope is not
  present after presentation-handle filtering.

### Verification

- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo test -p arcweft-runtime-driver --all-features component_handle_lifecycle_filters_runtime_controls`
- `cargo test -p arcweft-cli --all-features player_semantic_objects_preserve_runtime_component_parent`
- `cargo test -p arcweft-cli --all-features missing_requested_capture_scopes_report_structured_diagnostics`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The component scoped-capture cut was measured at Jujutsu change `qunnupmk`.
The structure audit reported 0 errors and 138 warnings. Relevant changed
production files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-bundle/src/resource_codec/ui/codec.rs` | 40,387 | 1,172 | production | false | UI codec public-id accounting |
| `crates/arcweft-bundle/src/resource_codec/ui/model.rs` | 50,684 | 1,672 | production | true | UI resource/runtime model and runtime projection |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | 37,097 | 1,051 | production | true | Native Agent observe object/component capture mapping |
| `crates/arcweft-cli/src/app/bundle.rs` | 77,725 | 2,159 | production | false | Legacy bundle/UI resource construction |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 47,867 | 1,356 | production | false | Component/View sidecar lowering |
| `crates/arcweft-runtime-driver/src/display.rs` | 36,396 | 974 | production | true | Bundle presentation snapshots and handle filtering |
| `crates/arcweft-runtime-driver/src/presentation_handles.rs` | 30,629 | 922 | production | true | Presentation handle state table and resource filters |
| `crates/arcweft-runtime-driver/src/session.rs` | 60,993 | 1,613 | production | false | Bundle session runtime bridge |

## Image Scoped Capture And Handle Visibility

- Runtime presentation-handle filtering for image handles is now covered by a
  lifecycle regression test. A live image handle mounts the matching
  `BundleImageObject`; `hide`, `unmount`, `release`, and `destroy` remove it
  from the presentation snapshot; `show` restores non-terminal hidden/unmounted
  handles.
- Agent player-backed image observation now has direct regression coverage for
  hidden image sources. Hidden image resources do not produce observed image
  objects and do not insert object frames into the Agent image frame store.
- The existing structured `AGENT_CAPTURE_MISSING_SCOPE` diagnostic therefore
  also covers requested image-object scopes after image handles filter the
  presentation snapshot.

### Verification

- `cargo test -p arcweft-runtime-driver --all-features image_handle_lifecycle_filters_presentation_images`
- `cargo test -p arcweft-cli --all-features player_image_object_observation_skips_hidden_source_and_frame`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The image scoped-capture cut was measured at Jujutsu change `ommtlxkq`. The
structure audit reported 0 errors and 138 warnings. Relevant changed production
files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | 39,949 | 1,129 | production | true | Native Agent observe image object/frame mapping |
| `crates/arcweft-runtime-driver/src/display.rs` | 40,435 | 1,090 | production | true | Bundle presentation snapshots and image handle filtering |

## Hidden Handle Input Rejection

- Player-scene now drops a focused runtime text editor when the next lowered
  runtime text-control set no longer contains that editor's session/target.
  This covers hidden, unmounted, released, and destroyed component handles after
  runtime-driver filtering removes their child text controls from the
  presentation snapshot.
- Direct platform text input and IME events are now accepted only while the
  current prepared frame still exposes the same focused text-input session and
  target. Stale events from a hidden/disposed control clear the local editor and
  produce no text-control writeback.
- Action-button `TextInputSubmit` activation now verifies that the submit
  target is present as a text input in the current frame before emitting a
  writeback. This prevents a stale render action from submitting a hidden input
  if a button survives independently.

### Verification

- `cargo test -p arcweft-player-scene --all-features hidden_runtime_text_control_clears_focus_and_rejects_stale_writeback`
- `cargo test -p arcweft-player-scene --all-features pointer_activation_rejects_submit_when_input_target_is_not_in_frame`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The hidden-handle input rejection cut was measured at Jujutsu change `nrkzpzql`.
The structure audit reported 0 errors and 138 warnings. Relevant changed files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-player-scene/src/input.rs` | 48,342 | 1,368 | production | true | Shared native/web input routing, focus, text editing, and writebacks |
| `crates/arcweft-player-scene/src/text_controls.rs` | 9,080 | 232 | production | false | Runtime text-control lowering and focus activation |
| `crates/arcweft-player-scene/tests/action_button_submit.rs` | 11,047 | 287 | test | false | Action-button submit and action invoke input regressions |
| `crates/arcweft-player-scene/tests/runtime_text_controls.rs` | 14,760 | 373 | test | false | Runtime text-control focus/editing regressions |

## Explicit Mount Canonicalization

- Expression-statement `image(...)` and `component(...)` calls now lower to the
  same `presentation.handle.create` effect family as value-position handles.
  The explicit form receives a deterministic lowering-owned handle id derived
  from the owner flow, mount kind, and mounted resource id.
- Explicit mounts default to lexical scope cleanup, matching value-position
  `component(...)` / `image(...)` handles. Authors can still opt out with the
  existing `lifetime = .manual`, `.detached`, or `.global` mount argument.
- Runtime presentation-handle create is idempotent for the same live handle id,
  kind, and resource. This keeps repeated explicit mount evaluation and flow
  re-entry stable while preserving the existing duplicate-id diagnostic for
  terminal handles or ids reused for a different resource.
- Runtime-plan label lowering now preserves unary and binary expression source
  labels for handle create arguments. This fixes `depth = -1000` and similar
  signed numeric presentation arguments that previously arrived at runtime as
  Rust AST debug text instead of executable argument text.

### Verification

- `cargo test -p arcweft-runtime-plan --all-features explicit_component_and_image_mount_exprs_lower_to_scoped_handle_create`
- `cargo test -p arcweft-runtime-driver --all-features create_is_idempotent_for_same_live_handle`
- `cargo test -p arcweft-runtime-plan --all-features value_position_component_handle_lowers_to_create_cleanup_and_close_cancel`
- `cargo test -p arcweft-runtime-plan --all-features`
- `cargo test -p arcweft-runtime-driver --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The explicit-mount canonicalization cut was measured at Jujutsu change
`ptwuyrsy`. The structure audit reported 0 errors and 138 warnings. Relevant
changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-runtime-driver/src/presentation_handles.rs` | 32,807 | 973 | production | true | Presentation handle parsing, lifecycle transitions, and runtime filtering |
| `crates/arcweft-runtime-plan/src/flow.rs` | 90,755 | 2,466 | production | false | Flow statement lowering and runtime operation planning |
| `crates/arcweft-runtime-plan/src/flow/presentation.rs` | 3,767 | 124 | production | false | Presentation handle lowering helpers |
| `crates/arcweft-runtime-plan/src/labels.rs` | 7,074 | 199 | production | false | Stable runtime-plan expression labels |
| `crates/arcweft-runtime-plan/src/flow/tests.rs` | 15,607 | 511 | test | true | Flow lowering regression tests |

## Image Handle Lifecycle And Agent Missing Scope Coverage

- Added runtime-plan coverage for value-position `let sprite = image(...)`
  handles. The regression now verifies that image handles lower to
  `presentation.handle.create`, register scoped disposal cleanup, bind the
  stable handle string, lower `show`, `hide`, and terminal `destroy` lifecycle
  methods to `presentation.handle.*`, and cancel the registered cleanup on the
  terminal operation.
- Added native Agent observe unit coverage for hidden image-object capture
  scopes. A hidden image source is not emitted as an observed object, its frame
  cache is not populated, and requesting that object scope reports the existing
  structured `AGENT_CAPTURE_MISSING_SCOPE` diagnostic.

### Verification

- `cargo test -p arcweft-runtime-plan --all-features value_position_image_handle_lowers_lifecycle_methods_and_cleanup_cancel`
- `cargo test -p arcweft-cli --all-features hidden_image_object_capture_scope_reports_missing_scope_diagnostic`
- `cargo test -p arcweft-runtime-plan --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The image-handle lifecycle and Agent missing-scope cut was measured at Jujutsu
change `ykzvqqzp`. The structure audit reported 0 errors and 138 warnings.
Relevant changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | 41,503 | 1,181 | production | true | Native Agent observe object/layer/component mapping and capture diagnostics |
| `crates/arcweft-runtime-plan/src/flow/tests.rs` | 18,114 | 593 | test | true | Flow lowering regression tests |

## Generic Callback Block Sugar

- Added expression-parser support for generic postfix callback block sugar.
  `expr.name { body }` now parses as a `MethodCall` whose single positional
  argument is a zero-argument `Closure`, matching the canonical callback
  spelling `expr.name(|| body)`.
- Added parameterized callback block support for the expression surface:
  `expr.name { item, index => body }` parses as a method call with a closure
  carrying the listed parameters. The parser recognizes the callback block
  generically after any postfix member name; type checking remains responsible
  for deciding whether the named member accepts a closure.
- The surface AST still preserves `Call` and `MethodCall` as distinct source
  shapes so later diagnostics and receiver-based resolution keep precise
  syntax evidence. A later HIR/typed lowering pass can still normalize both
  into one resolved call representation with `target`, optional `receiver`,
  arguments, and source-form metadata.
- This cut intentionally covers single-expression callback bodies. Multi
  statement callback bodies still need a later block-expression/statement parser
  integration so newline-sensitive Arcweft statements are preserved instead of
  being flattened by the expression lexer.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features postfix_callback_block`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The generic callback block sugar cut was measured at Jujutsu change
`kooomrzl`. The structure audit reported 0 errors and 138 warnings. Relevant
changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-lang-syntax/src/expr.rs` | 69,590 | 2,265 | production | true | Expression tokenization and Pratt parsing |
| `crates/arcweft-lang-syntax/tests/parser_p0.rs` | 18,644 | 630 | generated/test | false | Parser regression coverage |

## Entry/Test/Bench Goto Dispatch

- Removed `EntryItem::Start` and `EntryItem::Run` from the surface AST. Entry
  bodies now keep only `goto @flow...` as the structured flow dispatch item;
  removed `start` / `run` entry items recover as raw entry items with parser
  diagnostics that point authors to `goto @flow.name`.
- Updated semantic indexing, symbol collection, type checking, runtime-plan
  entry target lowering, compiler graph fixtures, samples, examples, and stable
  docs to use `goto @flow...` rather than entry-only `start` / `run` words.
- Updated script test and script bench launch extraction to use `goto @flow...`
  as well. Bench sections may write the canonical compact form
  `measure iterations = N { goto @flow.name }`; the runtime bench runner scans
  section bodies for that goto statement instead of parsing `start(@flow...)`.
- Direct script test/bench sources that have no explicit `entry` now use the
  first script manifest `goto @flow...` as the product-AWBC entry fallback, so
  headless script routes no longer need a separate entry-only start spelling.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features entry_goto`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-sema --all-features entry_`
- `cargo test -p arcweft-lang-sema --all-features script_tests`
- `cargo test -p arcweft-test --all-features`
- `cargo test -p arcweft-runtime-plan --all-features entry_`
- `cargo test -p arcweft-cli --all-features test_json_lists_script_tests -- --nocapture`
- `cargo test -p arcweft-cli --all-features bench_json_measures_headless_runtime_sections -- --nocapture`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The entry/test/bench goto cut was measured at Jujutsu change `ntmowtry`. The
structure audit reported 0 errors and 138 warnings. Relevant changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-cli/src/app/runtime/expectations.rs` | 7,938 | 263 | production | false | Script expectation and script goto target parsing |
| `crates/arcweft-cli/src/app/runtime/profile.rs` | 14,697 | 392 | production | false | Runtime profile compilation and script manifest entry fallback |
| `crates/arcweft-cli/src/app/runtime/script_bench/run.rs` | 17,853 | 507 | production | false | Script bench execution and assertion replay |
| `crates/arcweft-cli/src/app/runtime/script_bench/samples.rs` | 29,509 | 629 | production | false | Script bench section validation and flow target extraction |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | 53,074 | 2,217 | production | false | Surface item AST, including entry body items |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | 47,938 | 1,375 | production | false | Top-level item parsing and entry dispatch diagnostics |
| `crates/arcweft-lang-sema/src/project_index.rs` | 30,753 | 1,092 | production | false | Project graph relation kinds |
| `crates/arcweft-lang-sema/src/project_index/relations.rs` | 42,442 | 1,170 | production | false | Project graph relation indexing |
| `crates/arcweft-runtime-plan/src/flow.rs` | 90,672 | 2,464 | production | false | Runtime entry target lowering |

## Action Payload Signature Checking

- Component/View `action.invoke(...)` now preserves the authored payload field
  name in the syntax AST instead of keeping only the payload value. The payload
  name is stored compactly as `Box<str>` so the existing `ViewExpr` size profile
  does not regress.
- `ComponentViewBody` exposes a typed `action_invokes()` traversal so later
  sema/lowering layers can inspect action emit sites without reparsing View
  source strings or reaching into private AST fields.
- Type checking now builds a module-local signature registry from
  `pub action name(...)` declarations. Empty action declarations accept no
  payload; declared named payload parameters such as `value: String` or
  `name: String` are parsed through the existing function signature/type
  reference parser and currently validate against the UI payload
  representation, which is `String` for literal strings and text-control
  projections.
- Component action emits are checked against the declaration: undeclared action
  targets, wrong target families, unexpected payload names, missing required
  payloads, and payload type mismatches now produce type-check errors before
  bundle lowering.
- A temporary rejection test confirmed that
  `action.invoke(@action:.feedback.submit, payload = "ready")` is rejected when
  the declaration is `pub action feedback.submit(value: String)`. Per current
  test policy, that rejection test was removed after confirming the behavior.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features component_view_button_on_click_action_invoke_block_parses`
- `cargo test -p arcweft-lang-sema --all-features typechecks_component_action_invoke`
- `cargo test -p arcweft-cli --all-features component_view_action_invoke_button_lowers_to_action_resource`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The action payload signature checking cut was measured at Jujutsu change
`rqlxylyl` / commit `7289f89d`. The structure audit reported 0 errors and 138
warnings. Relevant changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 21,577 | 796 | production | false | Component/View AST action payload-name retention and traversal |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 36,271 | 994 | production | false | View action callback parsing and action payload-name capture |
| `crates/arcweft-lang-sema/src/checker.rs` | 30,439 | 819 | production | false | Type checker state and local action signature model |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 62,712 | 1,559 | production | false | Module-level action declaration signature collection and component emit validation |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 67,110 | 2,093 | test | false | Type-check coverage for matching action emit signatures |
| `crates/arcweft-lang-syntax/tests/style_component_view.rs` | 9,458 | 353 | test | false | Component/View parser coverage for action payload names |
| `crates/arcweft-lang-syntax/tests/parser_p0.rs` | 19,196 | 599 | test | false | Parser regression formatting cleanup |

## Multi-Statement Callback Blocks

- Postfix callback block sugar now preserves the raw source inside `{ ... }`
  before parsing the body. The expression parser attaches source spans to
  tokens so callback bodies no longer collapse newlines into a single lossy
  expression string.
- Callback block bodies now lower to the existing `Expr::Block { statements,
  value }` form. Single-expression callback blocks become an empty-statement
  block with a final value; multi-statement blocks preserve leading statements
  and the final expression value through the same parser path used by scope
  expressions.
- Parameterized callback blocks still use the same `item, index => body`
  surface. The parameter list remains parsed from top-level tokens, while the
  body is sliced from the original source and then parsed as an expression
  block.
- Component/View `.on_click { ... }` inline modifier blocks now parse through
  the same callback body path. Button activation therefore recognizes a final
  `action.invoke(...)`, `text_submit(...)`, or `noop` after earlier statements
  such as `let value = visitor_name.text`.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features postfix_callback_block`
- `cargo test -p arcweft-lang-syntax --all-features component_view_button_on_click_multi_statement_block_uses_final_action`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-sema --all-features typechecks_component_action_invoke`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The multi-statement callback block cut was measured at Jujutsu change
`mxwqzyrw`. The structure audit reported 0 errors and 138 warnings. Relevant
changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-lang-syntax/src/expr.rs` | 71,059 | 2,160 | production | true | Expression token spans and postfix callback block parsing |
| `crates/arcweft-lang-syntax/src/parser.rs` | 19,242 | 504 | production | false | Parser-facing callback block body bridge |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 36,625 | 1,006 | production | false | Component/View inline callback activation parsing |
| `crates/arcweft-lang-syntax/tests/parser_p0.rs` | 20,451 | 637 | test | false | Postfix callback block expression coverage |
| `crates/arcweft-lang-syntax/tests/style_component_view.rs` | 10,568 | 388 | test | false | Component/View multi-statement callback coverage |

## Reactive View Branching Surface

- Added canonical View builder parsing for ordinary `if`, `match`, and
  `for pattern in source key = expr` blocks. The parser now lowers those
  authoring forms into the existing internal `ViewIf`, `ViewMatch`, and
  `ViewForEach` AST nodes instead of introducing author-facing `ForEach`
  syntax.
- `} else {` and newline-separated `else {` forms are both normalized for
  View `if` blocks. Standalone `else` still produces a structured parser
  diagnostic.
- Component/View text-control input discovery now recurses through
  `if`/`match`/`for`/`await` View nodes, matching action-invoke traversal.
- Bundle UI sidecar lowering now preserves `ViewIf` and `ViewMatch` as
  `UiProgramInstruction::Branch` spans, and `ViewForEach` as
  `UiProgramInstruction::RepeatKeyed` with deterministic digest references for
  condition/source/key schemas.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features component_view_reactive_if_match_for_parse_to_structured_view_exprs`
- `cargo test -p arcweft-cli --all-features component_view_reactive_if_match_for_lower_to_ui_program_instructions`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The reactive View branching cut was measured at Jujutsu change `zxypvxtw`.
The structure audit reported 0 errors and 139 warnings. Relevant changed Rust
files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 24,816 | 1,061 | production | false | Component/View AST branching accessors and traversal |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 43,611 | 1,279 | production | false | Component/View element, modifier, and branching parser |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 56,649 | 1,614 | production | false | Component/View sidecar lowering and layout evidence |
| `crates/arcweft-lang-syntax/tests/style_component_view.rs` | 12,106 | 490 | test | false | Component/View parser coverage |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 28,488 | 920 | test | false | Bundle sidecar lowering coverage |

## Remaining Work

- End-to-end save subsystem wiring is split to
  `docs/reviews/requests/2026-07-06-seq-06.16.6.1-save-load-scoped-presentation-handles.md`.
  This cut verifies serde roundtrip and rollback substrate, not a full player
  save/load scenario.
- Native/web/observe parity tests still need a broader hidden/disposed-handle
  suite covering component handles, image handles, hit-test/focus/writeback
  behavior across actual adapters. Runtime-plan explicit mount regressions are
  covered by the canonicalization cut, but native/web adapter parity still
  needs direct smoke coverage.
- Lexical cleanup integration for overlay pop and scene transition needs the
  owning overlay/scene lifecycle operations to call the cleanup drain path.
- `Scroll` is now a typed resource and sidecar element, but scroll offsets,
  clipping, input routing, save/restore of scroll state, and native/web/observe
  parity tests still need the dedicated scroll runtime behavior slice.
- The final UI syntax direction still needs View-local `let` binding and
  await/pending builder integration from the broader input/scroll syntax
  request. Ordinary `if`, `match`, and `for` View branching is covered by this
  cut.
