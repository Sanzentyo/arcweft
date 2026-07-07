# Scoped Presentation Handles And Final UI Syntax - 2026-07-06

This cut implements the first production slice of scoped presentation resource
handles and final View authoring syntax.

## 2026-07-07 View Rename And Semantic Submit Update

The current canonical authoring surface is `view`, not `component`.

- Top-level retained UI declarations are authored as `pub view Name() { ... }`.
- Flow-side mounting uses `view(@view:.Name, ...)`; `component(...)` and
  `@component` are not canonical in the current syntax stack.
- Runtime UI ownership metadata now uses `view` fields and `root_view` in the
  UI resource model. No serde compatibility alias is added for the removed
  `component` field names in this internal refactor.
- Text-control submission samples no longer use `text_submit`. They declare a
  typed `pub action ...`, emit it from `Button(...).on_click { action.invoke(...) }`
  or `TextField(...).on_submit { action.invoke(...) }`, and receive it in flow
  with `let event = receive action(@action:.name)`.
- `TextField`, `TextArea`, and `SecureField` can now use handle-first authoring
  such as `let name = input.text(@input:.name, initial = "")` followed by
  `TextField(name).purpose(.name).enter_key(.next)`.
- Runtime text-control submit write-backs whose submit handler is an `action.*`
  id now resume the same `receive action(...)` wait path as player-rendered
  action buttons. Enter/IME submit and button click therefore converge on the
  same typed action route.

The lower-level layout capture mask contract now also uses
`CaptureScope::View`, so Agent protocol, CLI observe/capture APIs, and selected
capture metadata all agree on `view` rather than carrying an internal
`component` adapter term.

## 2026-07-07 View Terminology Cleanup

The current parser/HIR/sema/bundle test surface now uses canonical `view_*`
names rather than mechanically duplicated rename artifacts. Bundle View lowering
now names the body lowering function `lower_view_body`, and inline View style
sidecars use `ui.style.inline.view`.

Implementation notes that referred to removed intermediate body API names now
name the actual syntax type, `ViewBody`. This is a terminology cleanup only; it
does not add compatibility aliases or accept the removed `component` syntax.

Validation:

- `cargo test -p arcweft-lang-syntax --all-features --test style_view -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib view_ -- --nocapture`
- `cargo test -p arcweft-lang-sema --all-features view_text_control_inputs -- --nocapture`
- `cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-cli --all-targets --all-features`

## 2026-07-07 Generic Callback Block Sugar

View modifier parsing now accepts generic event callback sugar of the form
`.on_<event>(...)` and `.on_<event> { ... }`, not only the special `click` and
`submit` cases. The parser still lowers these to the existing typed
`ViewModifier::OnEvent { name, body }` shape, so `.on_click` and `.on_submit`
continue to feed button activation and text submit actions while other events
such as `.on_focus { ... }` preserve their callback block body.

Bundle View lowering now turns button event modifiers into
`ViewProgramInstruction::BindHandler` as ordinary View elements already did. This
keeps the final callback surface generic without adding a compatibility alias or
a separate event API.

Validation:

- `cargo test -p arcweft-lang-syntax --all-features --test style_view view_generic_callback_block_modifier_parses -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib view_generic_callback_block_lowers_to_handler_binding -- --nocapture`
- `cargo test -p arcweft-lang-syntax --all-features --test style_view -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib view_ -- --nocapture`
- `cargo check -p arcweft-lang-syntax -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-lang-syntax -p arcweft-cli --all-targets --all-features`

## 2026-07-07 Scroll Axis Contract

View-authored `Scroll(...)` has a canonical authoring default of vertical
scrolling. Authors can override it with `axis = .horizontal` directly or set an
equivalent `axis` through the matched style contract. The default is applied at
DSL lowering time, so runtime resources still carry an explicit typed axis.

The typed UI resource and runtime display contracts require
`content_width_milli`, `content_height_milli`, and `axis` on scroll regions.
Input snapshots require both `offset_x` and `offset_y` on each scroll offset
entry. No old serialized-resource defaulting path is retained for missing
`axis`, `content_width_milli`, or `offset_x`.

## 2026-07-07 Scroll-Contained View Images

`Image(@image:.id)` inside a View now resolves through the View hierarchy rather
than through image-declaration scroll annotations. `Scroll { ... }` is the View
element that owns the scroll container, offset state, and clipping contract.
`Image` remains a child View element; it does not acquire an author-facing
scroll option. When a View image element references an existing authored image
object, lowering clones that source image object into a View-retained display
item at the current layout cursor. If the element is nested inside
`Scroll { ... }`, that display item records the current scroll-region id only as
`containing_scroll_region`, the same internal containment reference used by
text controls and buttons.

Top-level `image` declarations and runtime inline image calls do not accept
user-authored `containing_scroll_region` / `view` ownership shortcuts. Scroll
containment is derived from retained View structure. Renderer and report paths
then apply the same scroll offset and viewport clipping used by runtime text
controls and action buttons:

- prepared frames drop scroll-contained images that are outside the viewport;
- partially visible images keep adjusted bounds plus a viewport clip;
- image quads adjust UVs to the visible sub-rectangle;
- Web frame reports and native Agent observation use the visible image bounds.

## 2026-07-07 Scroll-Contained View Text

`Text("...")` follows the same authoring rule as images and controls:
authors add `Scroll { ... }` as a View container, not a scroll option on the
text or image leaf. Lowering now emits retained `ViewTextBlockResource` entries
for View text. When a text element is nested under `Scroll { ... }`, the runtime
text block records the current scroll-region id only as an internal containment
reference after View layout has been flattened.

The session presentation snapshot carries `ViewRuntimeTextBlock` alongside
runtime controls, scroll regions, and images. View lifecycle handles filter text
blocks by their own id, target, and owning View id, so hiding or releasing a
View cannot leave retained text behind. The shared player frame path converts
runtime text blocks into `RenderTextBlock`, applies the containing scroll
offset and viewport clip, and then exposes the result through the existing
prepared-frame text list used by Web frame reports.

Current validation for this slice:

- `cargo test -p arcweft-cli --all-features --lib view_scroll_contains_nested`
- `cargo test -p arcweft-runtime-driver --all-features view_handle_lifecycle_filters_text_blocks -- --nocapture`
- `cargo test -p arcweft-player-scene --all-features --test scroll_regions player_frame_offsets_and_clips_scroll_contained_text_blocks -- --nocapture`
- `cargo test -p arcweft-bundle --all-features --test ui_resource_codecs ui_resource_ -- --nocapture`
- `cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene --all-targets --all-features`

## 2026-07-07 Review-Carried View/CSS/Scroll Policy

The 2026-07-07 review attachment has been folded into the repository as an
implementation policy and as the follow-up request
`docs/reviews/requests/2026-07-07-seq-06.16.6.4-view-resource-naming-taxonomy-and-css-scroll-policy.md`.

The policy recorded from that review is:

- `component` is no longer a public UI authoring term. Public syntax,
  diagnostics, samples, tests, Agent observe/capture, and layout capture use
  `view`, `@view`, and `root_view`.
- `Scroll` remains a View-tree structural element. Authors put image, text,
  button, and input leaves inside `Scroll { ... }`; they do not attach scroll
  attributes to those leaves.
- CSS style syntax targets Arcweft-owned View elements and typed style
  resources. It does not imply browser DOM/CSSOM fallback.
- CSS overflow properties configure Scroll behavior only when the matched
  element is an authored `Scroll`; non-Scroll leaves with interactive overflow
  should receive structured diagnostics.
- Flattened runtime/render resources may carry scroll containment metadata,
  but that metadata means "clipped and offset by an ancestor Scroll", not that
  the leaf owns scrolling.

The same request records the naming follow-up raised by the review: View-owned
Rust boundary types currently named `Ui*` should be audited and renamed to
`View*` unless they genuinely describe broader product-level UI catalogs such
as style, theme, input, or text sources. This is intentionally separate from
the already-recorded legacy `Component*` rename request, because the `Ui*`
taxonomy needs a precise split rather than a blanket mechanical replacement.

## 2026-07-07 View-Owned Resource Type Rename

The first View resource taxonomy slice now renames View-owned compact resource
and runtime boundary types from generic `Ui*` names to `View*` names without
adding aliases or serde compatibility shims.

Renamed types include:

- `ViewProgramResource`, `ViewProgramInstruction`, and related program metadata
  such as `ViewChildSpan`, `ViewHandlerRef`, `ViewSemanticTarget`, and
  `ViewStyleApplyRef`;
- retained View leaf/container resources such as `ViewLayoutBoundsResource`,
  `ViewScrollRegionResource`, `ViewTextBlockResource`,
  `ViewActionButtonResource`, `ViewFocusGroupResource`, and
  `ViewFocusNavigationResource`;
- runtime projection types such as `ViewRuntimeActionButton`,
  `ViewRuntimeTextBlock`, `ViewRuntimeScrollRegion`,
  `ViewRuntimeFocusGroup`, and `ViewRuntimeFocusNavigation`;
- View-owned policy enums such as `ViewScrollAxis`,
  `ViewScrollOverflowPolicy`, `ViewFocusDirection`,
  `ViewFocusTargetResolution`, `ViewFocusGroupPolicy`,
  `ViewFocusInitialPolicy`, `ViewFocusWrapPolicy`, and
  `ViewFocusSkipPolicy`.

Catalog-level UI resources intentionally keep `Ui*` names in this slice:
`UiStyleResource`, `UiThemeResource`, `UiTextResource`, and
`UiInputResource` still describe product-level UI sidecar/catalog sections
rather than a single retained View tree. Shared runtime text-control and
control-style types also keep `Ui*` names until the input/style catalog boundary
is redesigned.

Current validation for this slice:

- `cargo fmt --all`
- `cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene --all-targets --all-features`
- `cargo test -p arcweft-bundle --all-features --test ui_resource_codecs ui_resource_ -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib view_scroll_contains_nested -- --nocapture`
- `cargo test -p arcweft-runtime-driver --all-features view_handle_lifecycle_filters_text_blocks -- --nocapture`
- `cargo test -p arcweft-player-scene --all-features --test scroll_regions player_frame_offsets_and_clips_scroll_contained_text_blocks -- --nocapture`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 147 warnings after this slice.

## 2026-07-07 Scroll Containment Field Rename

Leaf resources and runtime/render projections now name ancestor Scroll
containment explicitly as `containing_scroll_region`. This replaces the prior
single-field `scroll_region` name on image objects, text controls, retained View
text blocks, action buttons, and renderer-facing control/image structs.

The plural `scroll_regions` lists and `ViewScrollRegionResource` /
`ViewRuntimeScrollRegion` names remain unchanged because those records are the
actual Scroll viewport resources. The new singular field name is only used on
leaves that are clipped and offset by an ancestor Scroll. No serde alias or
Rust compatibility builder is retained for the old singular field name.

Current validation for this slice:

- `cargo fmt --all`
- `cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native --all-targets --all-features`
- `cargo test -p arcweft-bundle --all-features --test ui_resource_codecs ui_resource_ -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib view_scroll_contains_nested -- --nocapture`
- `cargo test -p arcweft-player-scene --all-features --test scroll_regions player_frame_offsets_and_clips_scroll_contained_text_blocks -- --nocapture`
- `cargo test -p arcweft-render-wgpu --all-features --test geometry scroll_region -- --nocapture`
- `cargo test -p arcweft-player-web --all-features --test parity scroll -- --nocapture`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 147 warnings after this slice.

## 2026-07-07 View Style Rule Type Rename And Overflow Diagnostics

The second View resource taxonomy slice renamed the style-rule boundary types
that select or describe authored View elements:

- `ViewElementKind`, `ViewElementState`, and `ViewInteractionState`;
- `ViewStyleToken`, `ViewStyleRule`, `ViewPartStyleRule`,
  `ViewStyleSelector`, `ViewStyleSelectorPart`, `ViewStyleDeclaration`, and
  `ViewStyleValue`;
- `ViewEnvironmentPredicate`.

`UiStyleResource` remains the product-level style catalog. Its View-targeting
rule payloads now use `View*` names, so the catalog boundary and retained View
selector contract are no longer conflated. No `Ui*` compatibility aliases or
serde fallback names were added.

Scroll CSS compatibility also moved from a silent best-effort path to an
author-visible contract:

- `overflow-x: auto|scroll` on a `Scroll` style rule or inline `Scroll`
  modifier lowers to a horizontal `ViewScrollRegionResource`;
- `overflow-y: auto|scroll` and `overflow: auto|scroll` remain valid on
  `Scroll`;
- interactive `overflow`, `overflow-x`, or `overflow-y` on non-Scroll View
  elements now emits `AWF0617 view::interactive_overflow_requires_scroll`,
  telling authors to wrap the content in `Scroll { ... }` or move the
  property to the Scroll element.

Current validation for this slice:

- `cargo fmt --all`
- `cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native --all-targets --all-features`
- `cargo test -p arcweft-cli --all-features --lib view_scroll_ -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib interactive_overflow -- --nocapture`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 147 warnings after this slice.

## 2026-07-07 Retained View Program Substrate Rename

The next View resource taxonomy slice renamed the retained `arcweft-ui`
program substrate that directly represents Arcweft View DSL lowering output.
These names are not persisted compatibility surfaces, so the old `Ui*` Rust
symbols were removed rather than aliased:

- `UiProgram`, `UiProgramBuilder`, `UiProgramId`, and `UiInstruction` became
  `ViewProgram`, `ViewProgramBuilder`, `ViewProgramId`, and
  `ViewInstruction`;
- program payload records such as `UiElementSpec`, `UiTextSpec`,
  `UiImageSpec`, `UiCustomSpec`, `UiViewCall`, `UiBranch`, `UiRepeat`, and
  `UiInstructionRange` became the corresponding `View*` records;
- `UiPartId`, `UiPartExport`, `UiStableKey`, `UiExpressionId`,
  `UiStyleApply`, `UiStylePatchId`, `UiEventBindingSpec`,
  `UiSemanticSpec`, and `UiHandlerProgram` became `View*` records;
- Takumi adapter metadata now records `ViewProgramId` and `ViewPartId`.

This slice intentionally leaves broader product-level UI concepts such as
`UiError`, `UiStyle`, `UiTextSource`, `UiImageSource`, retained semantic
fragments, and shared runtime-control style types for later taxonomy decisions.
Those names are not a single authored View program boundary in the current
crate split.

Current validation for this slice:

- `cargo fmt --all`
- `cargo test -p arcweft-ui --all-features`
- `cargo test -p arcweft-takumi-adapter --all-features`
- `cargo check -p arcweft-ui -p arcweft-takumi-adapter --all-targets --all-features`
- `cargo clippy -p arcweft-ui -p arcweft-takumi-adapter --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `rg -n "\\bUi(Program|ProgramBuilder|Instruction|InstructionRange|ElementSpec|TextSpec|ImageSpec|CustomSpec|ViewCall|Branch|Repeat|StyleApply|StylePatchId|EventBindingSpec|SemanticSpec|HandlerProgram|PartId|PartExport|StableKey|ExpressionId)\\b|\\bUiProgramId\\b" crates\\arcweft-ui crates\\arcweft-takumi-adapter -g "*.rs"`

The structure audit reported 0 errors and 147 warnings after this slice.

## 2026-07-07 Final View Vocabulary Diagnostics

The parser accepts only the final built-in View container vocabulary:
`Panel`, `Box`, `Scroll`, `Row`, `Column`, and `Stack`. Unsupported View block
or expression heads now fall through the same generic unsupported-View-element
diagnostic path. No migration-specific branches or compatibility aliases are
kept for old authoring words.

Current validation for this slice:

- `cargo test -p arcweft-lang-syntax --all-features --test style_view`
- `cargo check -p arcweft-lang-syntax --all-targets --all-features`
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features`

## 2026-07-07 Presentation Lifecycle Surface Tightening

Flow-authored presentation handles now keep only the final public lifecycle
surface. `show`, `hide`, `unmount`, `release`, and `destroy` remain valid on
presentation handles; `pop` is accepted only for `overlay(...)` handles and
lowers to the same dispose operation used by overlay cleanup. The removed
`close` and `dispose` aliases are no longer typechecked as public DSL methods;
they fall through to the ordinary unknown-method diagnostic rather than a
migration-specific compatibility path.

`view(@view:.Name)` remains scoped by default, so samples do not need to spell
`lifetime = .scope` or call a manual close method to get deterministic cleanup.
The modern feedback sample now relies on lexical scope cleanup for its mounted
panel.

Current validation for this slice:

- `cargo check -p arcweft-runtime-plan -p arcweft-lang-sema -p arcweft-cli --all-targets --all-features`
- `cargo test -p arcweft-runtime-plan --all-features value_position_`
- `cargo test -p arcweft-lang-sema --all-features view_handle`
- `cargo test -p arcweft-lang-sema --all-features overlay_handle_pop`
- `cargo test -p arcweft-cli --all-features --test native_text_input_sample_sidecars`
- `cargo test -p arcweft-cli --all-features --test native_text_input_native_interactive_smoke seq06_16_3_submit_samples_share_player_backed_semantic_action_routes`
- `cargo run -p arcweft-cli --all-features -- check samples\modern-feedback-ui\src\main.arcw`

## 2026-07-07 Agent View Capture Contract

Agent observe/capture scope naming now follows the View terminology:

- Observation reports serialize `views` rather than `components`.
- The MCP-style resource endpoint is `views.json`.
- Capture scopes use `view.*` resource names and `AgentImageScope::View`.
- CLI observe uses `--view`; MCP capture arguments use `arguments.view`.
- REPL capture accepts `:capture view <id>` and emits `{ "kind": "view" }`.
- Missing-scope diagnostics report `view` as the requested scope kind.

No `--component`, `arguments.component`, `components.json`, or JSON
`"kind": "component"` compatibility path is retained in the Agent protocol /
CLI observe surface. The remaining `component` terms in this area are unrelated
URI escaping helper names such as `agent_uri_component`, not scoped UI resource
names.

Current validation for this slice:

- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-cli --all-targets --all-features`
- `cargo test -p arcweft-agent-protocol --all-features`

## 2026-07-07 Layout Capture Scope View Rename

The shared layout capture metadata now uses `CaptureScope::View` rather than
`CaptureScope::Component`. This removes the last scoped-capture adapter term
between Agent `--view` / `AgentImageScope::View` and selected-capture layout
metadata. No compatibility enum variant or serde alias is retained; serialized
layout capture scopes now emit `{ "kind": "view", ... }`.

Current validation for this slice:

- `cargo test -p arcweft-agent-protocol --all-features view_scope_serializes_and_scrubs_source_identity -- --nocapture`
- `cargo check -p arcweft-layout -p arcweft-agent-protocol -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-layout -p arcweft-agent-protocol -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `rg -n "CaptureScope::Component|LayoutCaptureScope::Component|\bComponent \{ id" crates tests -g "*.rs"`
- `cargo test -p arcweft-cli --all-features view_uri_capture_request_parses_scope_and_kind`
- `cargo test -p arcweft-cli --all-features observed_views_group_visible_objects_by_parent_id`
- `cargo test -p arcweft-cli --all-features native_view_capture_targets_select_member_objects`
- `cargo test -p arcweft-cli --all-features missing_requested_capture_scopes_report_structured_diagnostics`
- `cargo test -p arcweft-cli --all-features repl_cli_inspection_capture_target_is_structured`

## 2026-07-07 Workspace Validation Pass

After the layout capture scope rename and request split, the current workspace
validation passed with the final View syntax and scoped presentation handle
changes applied:

- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `rg -n --glob "*.arcw" -- "->\s*View|\btext_submit\s*(?:@|\()|\bstart\s*\(|\brun\s*\(|\bcomponent\s+|@component|component\(" samples examples tests web`
- `rg -n "component_view_|component_handle|runtime_component|explicit_component|typechecks_component|CaptureScope::Component|LayoutCaptureScope::Component|component scoped|component emit|Component action" crates tests -g "*.rs"`
- `rg -n "component view|UI component|component-scoped|@component|component\(|->\s*View|text_submit" docs\00-overview docs\01-language docs\02-runtime docs\03-presentation docs\04-tooling docs\schemas docs\examples -g "*.md"`

The structural audit reports 0 errors and 146 warnings. The search gates above
return no hits in the checked current-syntax, production/test code, and stable
doc ranges.

## 2026-07-07 Agent Hidden View Capture Filtering

Native Agent capture target selection now ignores objects marked
`visible = false` for viewport, layer, view, object, and rich-text fallback
captures. Hidden-only view scopes therefore do not produce `views` entries and
cannot be selected as native capture targets; requested hidden view scopes fall
through the existing structured `AGENT_CAPTURE_MISSING_SCOPE` diagnostic path.

Current validation for this slice:

- `cargo test -p arcweft-cli --all-features observed_views_drop_hidden_only_parent_scope`
- `cargo test -p arcweft-cli --all-features native_view_capture_targets_reject_hidden_member_objects`
- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-cli --all-targets --all-features`

## 2026-07-07 Web Hidden View Input Parity

The web player has crate-local parity smoke for hidden view-owned runtime text
controls and action buttons. These tests run through the same shared
`InputController` and prepared-frame hit/focus tables used by the web adapter.
They prove that once view-handle filtering removes a text control or button
from the prepared frame, stale text input emits no text-control writeback,
old button coordinates are no longer hittable, keyboard focus candidates do
not include the removed target, and stale pointer activation emits no semantic
action.

Current validation for this slice:

- `cargo test -p arcweft-player-web --all-features --test input`
- `cargo test -p arcweft-player-web --all-features web_hidden_runtime_text_control_rejects_stale_writeback`
- `cargo test -p arcweft-player-web --all-features hidden_view`
- `cargo check -p arcweft-player-web --all-targets --all-features`
- `cargo clippy -p arcweft-player-web --all-targets --all-features`

## 2026-07-07 Web Image Handle Report Parity

Web prepared-frame reporting now has a focused regression for image resources
that have been removed by presentation-handle lifecycle filtering. The
runtime-driver already proves that image handles drop `snapshot.images` on
hide, unmount, release, and destroy; the web regression proves the downstream
`WebFrameObservationReport` carries live image ids/bounds while reporting zero
images after the prepared frame no longer contains the image. This covers the
browser-side diagnostic/readback summary path without claiming pinned GPU PNG
baseline promotion.

Native Agent observe also has an explicit released-image-object missing-scope
diagnostic regression: when a requested image object is absent from the current
objects list and has no stored frame, `AGENT_CAPTURE_MISSING_SCOPE` is emitted
after presentation-handle filtering.

The `samples/image-animation.arcw` sample now includes an
`entry.image_sprite_released` entry that dispatches with final `goto` syntax to
an authored `let sprite = image(@image.sample.pulse_sprite)` handle and then
calls `sprite.release()`. The native Agent observe integration smoke selects
that entry, requests the stale image object scope, and verifies that the image
object is absent while the report carries `AGENT_CAPTURE_MISSING_SCOPE`.

The checked-in `web/demo.awfb` fixture was regenerated from `web/demo.arcw`
with the current AWFB schema after updating the fixture entry dispatch from
removed `start(@flow.main)` syntax to `goto @flow.main`. The parity test
expectation now follows the current display contract by observing the speaker
display text `ずんだガイド`.

Current validation for this slice:

- `cargo test -p arcweft-player-web --all-features --test parity web_frame_report_drops_released_image_handle_resources`
- `cargo test -p arcweft-cli --all-features released_image_object_capture_scope_reports_missing_scope_diagnostic`
- `cargo run -p arcweft-cli --all-features --quiet -- check samples\image-animation.arcw`
- `cargo test -p arcweft-cli --all-features --test check agent_observe_reports_missing_scope_for_released_image_handle_object`
- `cargo test -p arcweft-player-web --all-features --test parity`

## 2026-07-07 Web Authored Image Handle Lifecycle Runner Smoke

Web parity now also covers the authored image-handle runtime path, not only a
manually constructed prepared frame. The `arcweft-player-web` parity fixture
parses a small DSL source, lowers it through HIR/runtime-plan/Product AWBC,
adds a matching bundle image asset/object, starts `BundleSession` with a
selected flow, and then builds a Web frame observation report from the runner's
presentation snapshot. The smoke proves three lifecycle cases:

- `image(@image.card, lifetime = .manual)` remains visible after the flow
  returns.
- `image(..., lifetime = .manual)` followed by `sprite.release()` is absent
  from the Web frame report.
- default-scoped `image(@image.card)` is absent after flow return because the
  registered scoped cleanup disposes the handle.

This closes the Web runner evidence gap for authored released and scoped
disposed image handles while keeping exact GPU PNG promotion as a separate
pinned-baseline gate.

Current validation for this slice:

- `cargo fmt`
- `cargo test -p arcweft-player-web --all-features --test parity web_runner_drops_authored_released_and_scoped_disposed_image_handles -- --nocapture`
- `cargo test -p arcweft-player-web --all-features --test parity`
- `cargo check -p arcweft-player-web --all-targets --all-features`
- `cargo clippy -p arcweft-player-web --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 146 warnings after this slice.

## 2026-07-07 Web Authored View-Owned Control Runner Smoke

Web parity now covers retained resources owned by an authored `view(...)`
handle through the same Product AWBC runner path. The parity fixture parses a
small source with `view(@view.WebPanel, lifetime = .manual)`, `panel.release()`,
and default scoped `view(@view.WebPanel)` flows, attaches typed bundle UI
resources for a text control, action button, and scroll region owned by
`view.WebPanel`, then prepares the frame through `PlayerFramePlanner`, the same
shared path used by native, web, and Agent observation.

The smoke proves that the manual-live View handle exposes the View-owned text
input, action button, scroll region, hit targets, and keyboard focus targets;
the released View handle removes those controls and rejects stale text
writeback and stale pointer activation; and the default scoped View handle is
also absent after flow return because lexical cleanup disposes it.

Current validation for this slice:

- `cargo fmt`
- `cargo test -p arcweft-player-web --all-features --test parity web_runner_filters_authored_view_owned_controls_and_scroll_regions -- --nocapture`
- `cargo test -p arcweft-player-web --all-features --test parity`
- `cargo check -p arcweft-player-web --all-targets --all-features`
- `cargo clippy -p arcweft-player-web --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 146 warnings after this slice.

## 2026-07-07 Native Agent Authored Scroll Observe/Capture Parity

Native Agent observe now has authored `Scroll { ... }` parity coverage through a
temporary final-syntax source compiled by the normal CLI path. The regression
mounts `view(@view:.ScrollPanel, lifetime = .manual)`, waits with
`receive action(...)`, and observes the live frame through the player-backed
native Agent adapter.

The live observe report proves that:

- `view.ScrollPanel` is present as an Agent observed view with capture refs;
- Scroll-owned `TextField` and `Button` semantics keep
  `parent_id = "view.ScrollPanel"`;
- the input text value survives into Agent object text;
- a button fully below the authored Scroll viewport is absent from observed
  objects;
- the view raw capture ref can be read back through `--read-uri` with native
  image metadata scoped to `view.ScrollPanel`.

The same test also runs the released flow and verifies that requested
`--view view.ScrollPanel` and `--object input.feedback` scopes produce
structured `AGENT_CAPTURE_MISSING_SCOPE` diagnostics after handle filtering.

Validation:

- `cargo fmt`
- `cargo test -p arcweft-cli --all-features --test check agent_observe_reports_authored_scroll_view_capture_and_release_filtering -- --nocapture`
- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

## 2026-07-07 Scroll Viewport Sizing And Overflow Policy

Authored `Scroll` elements now have a compact resource-level viewport contract
instead of deriving every viewport from child content:

- `Scroll(id = @scroll:.body, width = 360px, height = 120px, overflow = .hidden)`
  lowers to a stable `ViewScrollRegionResource.public_id`, explicit logical
  viewport bounds, and a typed overflow policy.
- `width`/`w` and `height`/`h` accept pixel unit numbers or integer pixel
  values and lower through the existing logical milli-pixel resource model.
- `overflow`, `overflow_y`, and `overflow-y` accept `.auto`, `.scroll`, or
  `.hidden`; `clip = false` maps to the same non-scrollable hidden policy.
- The compact UI resource codec serializes non-default overflow values,
  runtime presentation snapshots carry the policy, and player-scene frame
  preparation maps it to `RenderScrollOverflow`.
- Hidden overflow reports a zero scroll range, so native/web wheel routing keeps
  the retained scroll offset at zero even when content is taller than the
  viewport.
- The parser also accepts View property modifiers such as `.width(220px)` and
  `.clip(false)` on ordinary modifier chains. For scroll containers with child
  blocks, the current canonical spelling keeps viewport policy in named
  `Scroll(...)` arguments so the line-based parser does not confuse property
  modifiers with callback/style blocks.

Validation for this slice:

- `cargo fmt`
- `cargo check -p arcweft-bundle -p arcweft-lang-syntax -p arcweft-cli -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features`
- `cargo test -p arcweft-lang-syntax --all-features view_box_and_scroll_parse_as_canonical_elements`
- `cargo test -p arcweft-cli --all-features view_box_and_scroll_lower_to_typed_ui_resources`
- `cargo test -p arcweft-bundle --all-features ui_resource_compact_sections_round_trip_with_deterministic_bytes`
- `cargo test -p arcweft-player-scene --all-features hidden_overflow_scroll_region_keeps_offset_at_zero`
- `cargo test -p arcweft-render-wgpu --all-features hidden_scroll_region_reports_no_scroll_range`
- `cargo clippy -p arcweft-bundle -p arcweft-lang-syntax -p arcweft-cli -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 146 warnings after this slice.

## 2026-07-07 Scroll Style Rule Layout Defaults

Authored `Scroll` viewport bounds and overflow policy can now be supplied
through the retained UI style resource as deterministic layout defaults.

- Simple `Scroll { ... }` style rules may set `width`, `height`, `overflow`,
  `overflow-y`, `overflow_y`, or `clip`.
- Style tokens resolve recursively for those properties, so layout defaults can
  be shared through existing `token(...)` declarations.
- The layout resolver deliberately ignores stateful, interaction, environment,
  descendant, and child selectors for viewport geometry. Scroll layout must not
  depend on transient hover/focus state.
- Authored `Scroll(...)` named arguments and View property/inline style
  modifiers are applied after stylesheet rules, so explicit local authoring
  overrides stylesheet defaults.
- Current Arcweft style values still use existing style value syntax such as
  `milli(512000)` and `text("hidden")`; bare style-side `512px` syntax is a
  separate style grammar extension, not part of this slice.

Validation:

- `cargo test -p arcweft-cli --all-features view_scroll_uses_style_rules_for_viewport_and_overflow_defaults -- --nocapture`
- `cargo test -p arcweft-cli --all-features view_box_and_scroll_lower_to_typed_ui_resources -- --nocapture`
- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-cli --all-targets --all-features`

## 2026-07-07 Scroll Axis Contract

Authored `Scroll(axis = .horizontal)` now lowers into a typed scroll-axis
contract instead of being treated as a vertical-only viewport.

- `ViewScrollRegionResource` and `ViewRuntimeScrollRegion` require explicit
  `content_width_milli`, `content_height_milli`, and `axis` fields. These are
  required contract fields; scroll resource payloads that omit them are invalid.
- `InputControllerSnapshot` scroll entries require both `offset_x` and
  `offset_y`. Missing `offset_x` is not accepted as a backward-compatible
  vertical-only save shape.
- Native/shared player input stores per-region x/y offsets. The existing wheel
  route projects wheel delta onto y for vertical scroll regions and onto x for
  horizontal scroll regions.
- `RenderScrollRegion` carries `content_width`, `content_height`, `offset_x`,
  `offset_y`, and `axis`; renderer control bounds/clips subtract the active
  axis offset before hit-test, semantic bounds, text clip, and control effect
  planning.
- View style and authoring can set `axis` through `Scroll(axis = .horizontal)`,
  `.axis(.horizontal)`, inline style, or simple `Scroll { axis = text("horizontal") }`
  style rules.

Validation:

- `cargo test -p arcweft-cli --all-features --lib view_scroll_axis_horizontal_lowers_to_typed_scroll_region -- --nocapture`
- `cargo test -p arcweft-player-scene --all-features --test scroll_regions horizontal_scroll_region_tracks_x_offset_and_snapshot -- --nocapture`
- `cargo test -p arcweft-render-wgpu --all-features --test geometry horizontal_scroll_region_offsets_and_clips_owned_text_controls -- --nocapture`
- `cargo test -p arcweft-player-web --all-features --test input wheel_input_updates_horizontal_scroll_region_under_pointer -- --nocapture`
- `cargo test -p arcweft-bundle --all-features --test ui_resource_codecs ui_resource_compact_sections_round_trip_with_deterministic_bytes -- --nocapture`
- `cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features`

## 2026-07-07 Retained UI View Boundary Rename

The retained `arcweft-ui` substrate now uses View terminology for its owned
boundary types instead of retaining Component names behind the final View
syntax:

- `component.rs` was replaced by `view.rs`.
- `ComponentId`, `ComponentSchemaId`, `RustComponentId`,
  `ComponentImplementation`, `ComponentDescriptor`, and `ComponentRegistry`
  became `ViewId`, `ViewSchemaId`, `RustViewId`, `ViewImplementation`,
  `ViewDescriptor`, and `ViewRegistry`.
- `UiInstruction::CallComponent(UiComponentCall)` became
  `UiInstruction::CallView(UiViewCall)`.
- `FragmentKind::Component` became `FragmentKind::View`.
- `EntityStore::component` became `EntityStore::view`.
- `ComponentStyleOverride` became `ViewStyleOverride`.
- Takumi metadata now carries `view` / `data-aw-view` / `aw-view` rather than
  `component` / `data-aw-component` / `aw-component`.

No compatibility aliases, deprecated re-exports, duplicate modules, or serde
field aliases were added. The remaining `component` tokens found by repository
search in code are unrelated path-component or graph-component terminology.

Validation:

- `cargo test -p arcweft-ui --all-features`
- `cargo test -p arcweft-takumi-adapter --all-features`
- `cargo check -p arcweft-character-ui -p arcweft-render-wgpu -p arcweft-render-native -p arcweft-runtime-host -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-ui -p arcweft-takumi-adapter -p arcweft-character-ui -p arcweft-render-wgpu -p arcweft-render-native -p arcweft-runtime-host -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `rg -n "ComponentId|ComponentRegistry|ComponentDescriptor|ComponentImplementation|RustComponentId|ComponentSchemaId|UiComponentCall|CallComponent|FragmentKind::Component|ComponentStyleOverride|DuplicateComponentPublicId|with_component|\\.component\\(|component:|data-aw-component|aw-component" crates -g "*.rs"`
- `rg -n "\\bcomponent\\b|\\bcomponents\\b|\\bComponent\\b|\\bComponents\\b" crates\\arcweft-ui crates\\arcweft-takumi-adapter -g "*.rs"`

## 2026-07-07 AwaitView Builder Instruction

View builder syntax now preserves `AwaitView(expr) { ... }` as structured
syntax and bundle UI program data rather than dropping it during lowering.

- The View parser recognizes `pending`, `ready`, `error`, and `denied`
  branches with ordinary patterns and View expression bodies.
- `ViewAwaitBranchKind` records the branch role in the syntax AST, and
  `View::text_control_inputs()` / action traversal continue through await
  branch bodies.
- `ViewProgramInstruction::Await` records the awaited source schema plus
  per-branch pattern schemas and instruction spans for pending/ready/error/
  denied bodies.
- Bundle View lowering emits the `Await` instruction and lowers each branch
  body into the same UI program instruction stream used by `if`, `match`, and
  `for`.
- The compact UI program codec round-trips the new instruction through the
  existing deterministic JSON transcript section.

Validation:

- `cargo test -p arcweft-lang-syntax --all-features view_await_parse_to_structured_branches -- --nocapture`
- `cargo test -p arcweft-cli --all-features view_await_lowers_to_ui_program_branch_spans -- --nocapture`
- `cargo test -p arcweft-bundle --all-features ui_resource_compact_sections_round_trip_with_deterministic_bytes -- --nocapture`

## 2026-07-07 Final View Syntax Source And Stable Docs Sweep

The final `view` syntax sweep now covers current authored `.arcw` files and
stable design/example documentation:

- `web/ime-player-rendered.arcw` now uses entry `goto` dispatch instead of the
  removed `start(...)` form.
- Stable UI documentation and examples under `docs/03-presentation/` and
  `docs/examples/` now spell retained UI declarations as `view`, not
  `component`, and no longer show legacy View return annotations.
- Current samples/examples/tests/web `.arcw` files have no remaining
  legacy component, text-submit, start/run entry dispatch, or View return
  annotation syntax hits.

Validation:

- `cargo run -p arcweft-cli --all-features --quiet -- check web\ime-player-rendered.arcw`
- `rg` checks over current samples/examples/tests/web for legacy component,
  text-submit, start/run entry dispatch, and View return annotation syntax.
- `rg` checks over stable docs for legacy component, View return annotation,
  and text-submit authoring examples.

The structure audit reported 0 errors and 146 warnings after this slice.

## 2026-07-07 Native Player Session Save/Load UX

`arcw run --runner native` now accepts:

- `--session-load <path>`
- `--session-save-out <path>`

The saved file is a native-player session envelope with schema
`arcweft.native_player_session` version `1`. It stores the portable
`BundleSession` save bytes together with `InputControllerSnapshot`, so
player-owned scroll offsets can round-trip alongside the runtime presentation
handle table, tombstones, rollback state, and cleanup stacks.

The UX intentionally rejects `--watch`, `--runner auto`, `--runner web`, and
`--runner headless` when session save/load flags are present. Native live patch
session save semantics need a separate timing contract; web/headless do not own
the native `InputControllerSnapshot` state. Save still inherits the
runtime-driver quiescence rule: pending host calls, text write-backs, waiting
action receives, host tasks, or task generation pins fail with the typed
`BundleSessionSaveError::NonQuiescent` reason rather than silently dropping
state.

Current validation for this slice:

- `cargo test -p arcweft-player-native --all-features native_player_session_save_pairs_runtime_and_input_snapshots`
- `cargo test -p arcweft-cli --all-features --test native_text_input_trace_cli runtime_run_session_save_flags_are_native_player_only`
- `cargo run -p arcweft-cli --all-features --quiet -- run --runner headless samples\modern-feedback-ui\src\main.arcw --session-save-out target\arcweft\session-smoke.awfs` (expected native-only rejection)

## 2026-07-07 Button Submit Route Removal

Button activation now has a single final-syntax route:

- `ViewActionButtonActionResource`, `ViewRuntimeActionButtonAction`, and
  `RenderActionButtonAction` retain `Noop` and `ActionInvoke` only.
- The old Button-owned text-input submit action and IME policy were removed
  from bundle resources, render geometry, player-scene input routing, and
  tests. Button activation no longer creates text-control writebacks.
- Text control submit remains owned by the text control itself. Enter/IME
  submit writebacks resolve the authored submit handler, and `action.*`
  handlers resume the `receive action(...)` wait path.
- The runtime-driver no longer captures `ui.text_input.await_submit` host calls
  or reports `WaitingTextSubmitCalls` save blockers. Typed action waits use
  `ui.action.await`.

Current validation for this slice:

- `cargo test -p arcweft-bundle --all-features runtime_action_button`
- `cargo test -p arcweft-player-scene --all-features action_button`
- `cargo test -p arcweft-render-wgpu --all-features runtime_control`
- `cargo test -p arcweft-runtime-driver --all-features session_accepts_generic_semantic_action_invoke`
- `cargo test -p arcweft-runtime-driver --all-features session_receive_action_host_call_resumes_with_event_value`
- `cargo test -p arcweft-runtime-driver --all-features write_back_updates_runtime_overlay_and_returns_typed_event`
- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-runtime-driver -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 140 warnings.

## 2026-07-07 View-Owned Focus Resources

Focus resources now participate in scoped View lifecycle filtering:

- `ViewFocusGroupResource`, `ViewRuntimeFocusGroup`,
  `ViewFocusNavigationResource`, and `ViewRuntimeFocusNavigation` carry an optional
  `view` owner id.
- View lowering writes the owning `view.*` id into authored focus groups and
  navigation targets.
- Runtime presentation filtering now checks that owner view alias alongside
  the focus group's or navigation target's own id. Hidden, unmounted, released,
  or destroyed view handles therefore remove their focus groups/navigation from
  the presentation snapshot.
- Hidden focus diagnostics also consider the owner view alias, so stale
  navigation under a hidden view reports `HiddenButFocusable` before filtering.

Current validation for this slice:

- `cargo test -p arcweft-runtime-driver --all-features view_handle_lifecycle_filters_focus_resources`
- `cargo test -p arcweft-bundle --all-features ui_focus_navigation_compact_round_trip_is_deterministic`
- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 142 warnings.

## 2026-07-07 Overlay/Menu Cleanup Integration

Overlay and menu presentation handles now use the same scoped cleanup path as
View and image handles:

- Value-position and explicit `menu(...)` / `overlay(...)` calls lower to
  `presentation.handle.create`, a stable handle id, and scoped
  `presentation.handle.dispose` cleanup registration by default.
- Overlay handle method `pop()` lowers to `presentation.handle.dispose` and
  cancels the registered cleanup. Removed `close()` / `dispose()` aliases are
  no longer public DSL methods.
- A core engine regression now verifies that a scoped overlay cleanup drains
  during `goto`, which is the current flow-level scene transition path.

Current validation for this slice:

- `cargo test -p arcweft-runtime-plan --all-features value_position_overlay_handle_lowers_pop_to_dispose_and_cleanup_cancel`
- `cargo test -p arcweft-runtime-plan --all-features explicit_menu_and_overlay_mount_exprs_lower_to_scoped_handle_create`
- `cargo test -p arcweft-core --all-features scoped_overlay_cleanup_drains_on_goto_scene_transition`
- `cargo check -p arcweft-core -p arcweft-runtime-plan --all-targets --all-features`
- `cargo clippy -p arcweft-core -p arcweft-runtime-plan --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 143 warnings.

## Implemented

- Added runtime `FlowOp::RegisterCleanup` and `FlowOp::CancelCleanup` support.
  The structured engine records cleanup effects on the active lexical scope,
  drains them in LIFO order on scope exit, drains root cleanups on flow return or
  goto, and allows explicit cancellation for manual release/dispose paths.
- Added AWBC instruction, codec, verifier, VM, and product parity support for
  cleanup registration and cancellation.
- Lowered value-position `let panel = view(...)` and `let image =
  image(...)` calls to `presentation.handle.create`, scoped cleanup
  registration, and a stable string handle binding.
- Lowered handle methods `show`, `hide`, `unmount`, `release`, `destroy`, and
  overlay `pop` to `presentation.handle.*` effects. Terminal methods cancel the
  registered cleanup; removed `close` / `dispose` aliases fall through to the
  ordinary unknown-method diagnostic.
- Split presentation-handle helper logic into
  `crates/arcweft-runtime-plan/src/flow/presentation.rs` so the main flow
  lowerer stays below the structure-audit error threshold.
- Changed current View authoring syntax to `view Name() { ... }`
  with canonical `Panel`, `Column`, `Row`, and `Stack` elements. Removed
  legacy View return annotations and noncanonical container aliases as accepted
  View body syntax; unsupported words now use the same generic parse recovery
  as any other unsupported View element or expression head.
- Added canonical `Box` and `Scroll` View element vocabulary to parser,
  bundle-side `ViewElementKind`, style selectors, and View sidecar
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
- `cargo test -p arcweft-runtime-plan --all-features value_position_view_handle_lowers_to_create_cleanup_and_release_cancel`
- `cargo test -p arcweft-runtime-plan --all-features awbc_product_parity_scope_cleanup_and_cancel`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-syntax --all-features view_box_and_scroll_parse_as_canonical_elements`
- `cargo test -p arcweft-lang-sema --all-features`
- `cargo test -p arcweft-core --all-features fiber_checkpoint_and_serde_preserve_cleanup_stacks`
- `cargo test -p arcweft-runtime-driver --all-features presentation`
- `cargo test -p arcweft-cli --all-features view_box_and_scroll_lower_to_typed_ui_resources`
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

- View `Button(...).on_click { action.invoke(@action:.name, value = expr) }`
  now parses into a typed `ViewAction::ActionInvoke` activation. The parser
  accepts the block callback form and normalizes both call-shaped
  `action.invoke(...)` and method-call-shaped `action.invoke(...)` expression
  ASTs into the same action node.
- `ViewActionButtonActionResource`, `ViewRuntimeActionButtonAction`, and
  `RenderActionButtonAction` now carry `ActionInvoke { action, payload }`
  as the typed Button activation route. Runtime action-button lowering
  validates the authored action public id before rendering.
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

- `cargo test -p arcweft-lang-syntax --all-features view_button_on_click_action_invoke_block_parses`
- `cargo test -p arcweft-bundle --all-features runtime_action_button_resolves_action_invoke_action`
- `cargo test -p arcweft-player-scene --all-features pointer_activation_on_action_invoke_button_emits_semantic_action`
- `cargo test -p arcweft-runtime-driver --all-features session_accepts_generic_semantic_action_invoke`
- `cargo test -p arcweft-cli --all-features view_action_invoke_button_lowers_to_action_resource`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The action-invoke cut was measured at Jujutsu change `nqnzzvoz` /
`39e9c9c5`. The current structure audit still reports 0 errors and 138
warnings. Relevant changed production files:

| Path | Bytes | LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 35,170 | 967 | production | View element/modifier parsing and action callback normalization |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 19,257 | 726 | production | Typed View AST, including button activation payloads |
| `crates/arcweft-bundle/src/resource_codec/ui/model.rs` | 49,503 | 1,480 | production with embedded tests | UI resource/runtime model and runtime projection |
| `crates/arcweft-bundle/src/resource_codec/ui/codec.rs` | 39,725 | 1,080 | production | UI resource codec reference accounting |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 46,123 | 1,227 | production | View sidecar lowering into bundle resources |
| `crates/arcweft-player-scene/src/action_buttons.rs` | 5,116 | 130 | production | Runtime action-button resource lowering |
| `crates/arcweft-player-scene/src/input.rs` | 46,658 | 1,245 | production with embedded tests | Routed input, focus, text editing, and action-button activation |
| `crates/arcweft-render-wgpu/src/geometry/action_buttons.rs` | 8,460 | 227 | production | Action-button render geometry and semantic node emission |
| `crates/arcweft-runtime-driver/src/session.rs` | 58,208 | 1,433 | production with embedded tests | Bundle session input queueing and runtime bridge |

Relevant changed test files:

| Path | Bytes | LOC |
| --- | ---: | ---: |
| `crates/arcweft-bundle/tests/ui_action_button_resources.rs` | 3,969 | 100 |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 26,667 | 790 |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | 9,207 | 346 |
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
  `ViewActionPayloadResource`. `LiteralString` now represents authored string
  literals, and `TextControlProjection { input, field }` represents `.text` or
  `.value` projections from runtime text-control handles.
- View syntax records `ViewActionPayload` instead of raw source text.
  The parser accepts literal strings and text-control projections for
  `action.invoke(..., value = ...)`; unsupported expressions are not silently
  preserved as payload source strings.
- View sidecar lowering normalizes shorthand projections such as
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

- `cargo test -p arcweft-lang-syntax --all-features view_button_on_click_action_invoke_block_parses`
- `cargo test -p arcweft-bundle --all-features runtime_action_button_resolves_action_invoke_action`
- `cargo test -p arcweft-cli --all-features view_action_invoke_button_lowers_to_action_resource`
- `cargo test -p arcweft-player-scene --all-features runtime_action_invoke_payload_reads_text_control_projection`
- `cargo test -p arcweft-player-scene --all-features pointer_activation_on_action_invoke_button_emits_semantic_action`
- `cargo test -p arcweft-runtime-driver --all-features session_accepts_generic_semantic_action_invoke`
- `cargo test -p arcweft-runtime-driver --all-features session_receive_action_host_call_resumes_with_event_value`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The typed-payload cut kept the structure audit at 0 errors and 138 warnings.

## View Scoped Capture And Handle Visibility

- UI resource metadata now carries the owning view id from View
  lowering into `ViewSemanticTarget`, `UiInputOptions`, `ViewActionButtonResource`,
  `UiRuntimeTextControl`, and `ViewRuntimeActionButton`. The field is optional so
  non-view-owned resources keep the existing top-level behavior.
- Runtime presentation-handle filtering now treats a live view handle id as
  an alias for its owned runtime text controls and action buttons. Hiding,
  unmounting, releasing, or destroying a view handle removes those child
  controls from the presentation snapshot; showing the handle restores them.
- Agent native observe now preserves view ownership for runtime semantic
  objects by mapping prepared text-input and button targets back to their owning
  view ids. View grouping therefore reports the authored view
  scope instead of falling back to each object id.
- Agent observe now emits structured `AGENT_CAPTURE_MISSING_SCOPE` diagnostics
  when a requested `--view`, `--object`, or `--layer` capture scope is not
  present after presentation-handle filtering.

### Verification

- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo test -p arcweft-runtime-driver --all-features view_handle_lifecycle_filters_runtime_controls`
- `cargo test -p arcweft-cli --all-features player_semantic_objects_preserve_runtime_view_parent`
- `cargo test -p arcweft-cli --all-features missing_requested_capture_scopes_report_structured_diagnostics`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The view scoped-capture cut was measured at Jujutsu change `qunnupmk`.
The structure audit reported 0 errors and 138 warnings. Relevant changed
production files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-bundle/src/resource_codec/ui/codec.rs` | 40,387 | 1,172 | production | false | UI codec public-id accounting |
| `crates/arcweft-bundle/src/resource_codec/ui/model.rs` | 50,684 | 1,672 | production | true | UI resource/runtime model and runtime projection |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | 37,097 | 1,051 | production | true | Native Agent observe object/view capture mapping |
| `crates/arcweft-cli/src/app/bundle.rs` | 77,725 | 2,159 | production | false | Legacy bundle/UI resource construction |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 47,867 | 1,356 | production | false | View sidecar lowering |
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
- Button activation no longer emits text-control writebacks directly. Buttons
  either emit typed semantic actions or no action, while Enter/IME submit stays
  owned by the focused text control's submit handler. This removes the stale
  Button-to-text-submit path instead of keeping a second rejection gate.

### Verification

- `cargo test -p arcweft-player-scene --all-features hidden_runtime_text_control_clears_focus_and_rejects_stale_writeback`
- `cargo test -p arcweft-player-scene --all-features pointer_activation_on_noop_button_does_not_emit_action_or_write_back`
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

- Expression-statement `image(...)` and `view(...)` calls now lower to the
  same `presentation.handle.create` effect family as value-position handles.
  The explicit form receives a deterministic lowering-owned handle id derived
  from the owner flow, mount kind, and mounted resource id.
- Explicit mounts default to lexical scope cleanup, matching value-position
  `view(...)` / `image(...)` handles. Authors can still opt out with the
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

- `cargo test -p arcweft-runtime-plan --all-features explicit_view_and_image_mount_exprs_lower_to_scoped_handle_create`
- `cargo test -p arcweft-runtime-driver --all-features create_is_idempotent_for_same_live_handle`
- `cargo test -p arcweft-runtime-plan --all-features value_position_view_handle_lowers_to_create_cleanup_and_release_cancel`
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

## Generic Callback Action Inventory

- `ViewBody::action_invokes()` now returns an owned typed action-invoke
  inventory instead of references only to button activation and text-submit
  slots. This lets generic View callback modifiers such as
  `.on_focus { action.invoke(...) }` participate in the same sema action
  signature checks as `.on_click` and `.on_submit`.
- `Button` `.on_click` and text control `.on_submit` still use their canonical
  activation/submit extraction path, while other `.on_*` modifiers are scanned
  from their callback body. This avoids duplicate action records for the
  canonical click/submit paths without treating focus/hover/custom events as
  untyped raw callback payloads.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features --test style_view view_generic_callback_block_modifier_parses -- --nocapture`
- `cargo test -p arcweft-lang-sema --all-features generic_view_callback -- --nocapture`

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

- View `action.invoke(...)` now preserves the authored payload field
  name in the syntax AST instead of keeping only the payload value. The payload
  name is stored compactly as `Box<str>` so the existing `ViewExpr` size profile
  does not regress.
- `ViewBody` exposes a typed `action_invokes()` traversal so later
  sema/lowering layers can inspect action emit sites without reparsing View
  source strings or reaching into private AST fields.
- Type checking now builds a module-local signature registry from
  `pub action name(...)` declarations. Empty action declarations accept no
  payload; declared named payload parameters such as `value: String` or
  `name: String` are parsed through the existing function signature/type
  reference parser and currently validate against the UI payload
  representation, which is `String` for literal strings and text-control
  projections.
- View action emits are checked against the declaration: undeclared action
  targets, wrong target families, unexpected payload names, missing required
  payloads, and payload type mismatches now produce type-check errors before
  bundle lowering.
- A temporary rejection test confirmed that
  `action.invoke(@action:.feedback.submit, payload = "ready")` is rejected when
  the declaration is `pub action feedback.submit(value: String)`. Per current
  test policy, that rejection test was removed after confirming the behavior.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features view_button_on_click_action_invoke_block_parses`
- `cargo test -p arcweft-lang-sema --all-features typechecks_view_action_invoke_payload_signature`
- `cargo test -p arcweft-cli --all-features view_action_invoke_button_lowers_to_action_resource`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The action payload signature checking cut was measured at Jujutsu change
`rqlxylyl` / commit `7289f89d`. The structure audit reported 0 errors and 138
warnings. Relevant changed Rust files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 21,577 | 796 | production | false | View AST action payload-name retention and traversal |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 36,271 | 994 | production | false | View action callback parsing and action payload-name capture |
| `crates/arcweft-lang-sema/src/checker.rs` | 30,439 | 819 | production | false | Type checker state and local action signature model |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 62,712 | 1,559 | production | false | Module-level action declaration signature collection and view emit validation |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 67,110 | 2,093 | test | false | Type-check coverage for matching action emit signatures |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | 9,458 | 353 | test | false | View parser coverage for action payload names |
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
- View `.on_click { ... }` inline modifier blocks now parse through
  the same callback body path. Button activation therefore recognizes a final
  `action.invoke(...)` or `noop` after earlier statements
  such as `let value = visitor_name.text`.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features postfix_callback_block`
- `cargo test -p arcweft-lang-syntax --all-features view_button_on_click_multi_statement_block_uses_final_action`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-sema --all-features typechecks_view_action_invoke_payload_signature`
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
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 36,625 | 1,006 | production | false | View inline callback activation parsing |
| `crates/arcweft-lang-syntax/tests/parser_p0.rs` | 20,451 | 637 | test | false | Postfix callback block expression coverage |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | 10,568 | 388 | test | false | View multi-statement callback coverage |

## Reactive View Branching Surface

- Added canonical View builder parsing for ordinary `if`, `match`, and
  `for pattern in source key = expr` blocks. The parser now lowers those
  authoring forms into the existing internal `ViewIf`, `ViewMatch`, and
  `ViewForEach` AST nodes instead of introducing author-facing `ForEach`
  syntax.
- `} else {` and newline-separated `else {` forms are both normalized for
  View `if` blocks. Standalone `else` still produces a structured parser
  diagnostic.
- View text-control input discovery now recurses through
  `if`/`match`/`for`/`await` View nodes, matching action-invoke traversal.
- Bundle UI sidecar lowering now preserves `ViewIf` and `ViewMatch` as
  `ViewProgramInstruction::Branch` spans, and `ViewForEach` as
  `ViewProgramInstruction::RepeatKeyed` with deterministic digest references for
  condition/source/key schemas.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features view_reactive_if_match_for_parse_to_structured_view_exprs`
- `cargo test -p arcweft-cli --all-features view_reactive_if_match_for_lower_to_ui_program_instructions`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The reactive View branching cut was measured at Jujutsu change `zxypvxtw`.
The structure audit reported 0 errors and 139 warnings. Relevant changed Rust
files:

| Path | Bytes | LOC | Classification | Embedded Tests | Responsibility |
| --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-lang-syntax/src/ast/view.rs` | 24,816 | 1,061 | production | false | View AST branching accessors and traversal |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | 43,611 | 1,279 | production | false | View element, modifier, and branching parser |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 56,649 | 1,614 | production | false | View sidecar lowering and layout evidence |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | 12,106 | 490 | test | false | View parser coverage |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 28,488 | 920 | test | false | Bundle sidecar lowering coverage |

## View-Local Input Handle Let Binding

- Added View-local `let name = expr` parsing for View bodies. The
  parser records the binding as `ViewExpr::Let` rather than treating it as a
  custom element or raw line, so later lowering can consume handle builders
  without reparsing source strings.
- Added direct input-handle discovery for `input.text(@input:.id, initial =
  "...")` and `input.secure(@input:.id, initial = "...")` builder values.
  View `text_control_inputs()` now reports these handles even when the
  following `TextField(name)` uses the local handle instead of spelling the
  input id inline.
- Added `ViewProgramInstruction::BindLocal` so bundle UI programs preserve the
  authored local binding with deterministic pattern/value schema digests. This
  gives runtime-plan cleanup and later pending/await builder work a typed
  instruction to extend instead of a stringly custom element.
- Bundle View lowering now resolves `TextField(local_name)` through
  the preceding input-handle binding. The generated `UiInputOptions.public_id`
  uses the authored input id, and the initial text source uses the builder's
  displayed initial value instead of the local variable name.

### Verification

- `cargo fmt`
- `cargo test -p arcweft-lang-syntax --all-features view_local_let_input_handle_parses`
- `cargo test -p arcweft-cli --all-features view_local_let_input_handle_lowers_to_program_binding`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The structure audit reported 0 errors and 139 warnings after this slice.

## Text-Control Submit Actions

- Added View parser support for `.on_submit { action.invoke(...) }` and
  `.on_submit(|| action.invoke(...))` on `TextField`, `TextArea`, and
  `SecureField` nodes. The parsed submit route uses the same `ViewAction`
  representation as button `.on_click`.
- Added `.purpose(...)` and `.enter_key(...)` text-control modifiers so
  handle-first authoring can keep heads small: `TextField(name)` can now carry
  typed options through modifiers instead of head arguments.
- Bundle View lowering now maps submit actions to
  `UiInputOptions.submit_handler = "action.*"`. Existing default text-control
  writeback handlers remain
  available for controls without an authored action route.
- Runtime-driver text-control submit write-backs whose handler id is an
  `action.*` id now resume pending `receive action(...)` waits with the
  submitted text value. This makes Enter/IME submit and player-rendered button
  activation converge on the same flow-side action receive primitive.
- Updated `samples/modern-feedback-ui`, `samples/text-submit-flow`,
  `samples/native-text-input`, and `samples/focus-navigation-controller-dsl` to
  use `view`, handle-first text controls, typed actions, `action.invoke`, and
  `receive action` rather than `text_submit`.

### Verification

- `cargo check -p arcweft-cli --all-targets --all-features`
- `cargo test -p arcweft-lang-syntax --all-features --test style_view`
- `cargo test -p arcweft-cli --all-features view_button_lowers_to_action_button_sidecar`
- `cargo test -p arcweft-runtime-driver --all-features session_text_control_submit_handler_resumes_receive_action`
- `cargo test -p arcweft-cli --all-features --test native_text_input_sample_sidecars`
- `cargo test -p arcweft-cli --all-features --test native_text_input_native_interactive_smoke`
- `cargo run -p arcweft-cli --all-features -- check samples\modern-feedback-ui\src\main.arcw`
- `cargo run -p arcweft-cli --all-features -- check samples\text-submit-flow\src\main.arcw`
- `cargo run -p arcweft-cli --all-features -- check samples\native-text-input\src\main.arcw`
- `cargo run -p arcweft-cli --all-features -- check samples\focus-navigation-controller-dsl\src\main.arcw`

## Scroll Runtime Input Substrate

- Added `RenderScrollRegion` to the shared renderer/player scene contract and
  preserved those regions through `PreparedFrame` planning and viewport-fit
  mapping. This gives native, web, and Agent observation a common region list
  instead of each adapter inventing scroll hit boxes.
- Added `ViewScrollRegionResource` and `ViewRuntimeScrollRegion` to the compact UI
  program contract. Authored View `Scroll { ... }` elements now lower into a
  deterministic scroll-region resource owned by the surrounding `view.*`
  handle, round-trip through compact resource encoding, flow through the bundle
  session runtime snapshot, and are filtered by presentation-handle lifecycle in
  the same pass as text inputs, buttons, and focus navigation resources.
- Player scene frame preparation now maps runtime scroll regions into
  `RenderScrollRegion` values using the current input controller offset. This
  connects authored `Scroll` views to prepared-frame hit testing without adding
  native/web adapter-specific geometry rules.
- Scroll containment now flows through `UiInputOptions`,
  `ViewActionButtonResource`, `UiRuntimeTextControl`, `ViewRuntimeActionButton`,
  image objects, retained text blocks, and the renderer-facing control/image
  structs as a typed `containing_scroll_region` reference. View lowering
  maintains a lexical scroll stack so leaves authored inside `Scroll` receive
  that region id directly rather than being assigned by bounds inference.
- Shared frame planning offsets scroll-owned text controls and action buttons
  by the region's clamped `offset_y`, clips their paint/text bounds to the
  scroll viewport, and emits hit-test/semantic bounds only for the visible
  portion. Controls whose scroll region is missing or fully outside the
  viewport are omitted from the prepared frame.
- Runtime-control backdrop, foreground filter, shadow pass, and paint bounds
  now use the visible scroll-clipped control bounds. This keeps effect plans
  aligned with the same viewport portion used for hit-test and semantic bounds.
- Added `InputController`-owned vertical scroll offsets keyed by scroll region
  id. Wheel input now routes through the latest prepared frame, selects the
  topmost scroll region under the current pointer position, clamps the offset to
  the region's content height, and leaves existing choice scroll state
  unchanged.
- `InputControllerSnapshot` now serializes and restores choice scroll plus
  scroll-region offsets with validation for empty region ids and non-finite or
  negative offsets. Player-scene frame preparation can therefore restore a live
  scroll position without re-deriving it from wheel history.
- Native and web wheel event paths now pass their prepared frame into the
  shared input controller. Native also re-prepares the frame after wheel input
  so later render-offset integration does not need an adapter-specific update
  path.

### Verification

- `cargo test -p arcweft-player-scene --all-features wheel_updates_scroll_region_under_pointer_and_clamps`
- `cargo test -p arcweft-player-web --all-features wheel_input_updates_scroll_region_under_pointer`
- `cargo test -p arcweft-render-wgpu --all-features scroll_regions_survive_frame_planning_and_viewport_mapping`
- `cargo test -p arcweft-cli --all-features view_box_and_scroll_lower_to_typed_ui_resources`
- `cargo test -p arcweft-bundle --all-features ui_resource_compact_sections_round_trip_with_deterministic_bytes`
- `cargo test -p arcweft-runtime-driver --all-features view_handle_lifecycle_filters_scroll_regions`
- `cargo test -p arcweft-player-scene --all-features player_frame_plans_runtime_scroll_regions_and_applies_input_offset`
- `cargo test -p arcweft-render-wgpu --all-features scroll_region_offsets_and_clips`
- `cargo test -p arcweft-player-scene --all-features scroll`
- `cargo test -p arcweft-render-wgpu --all-features scroll_region_`
- `cargo check -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-player-web -p arcweft-player-native -p arcweft-render-wgpu -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 144 warnings after this slice.

## Product AWBC Flow and Image Metadata Slice

- Product AWBC session startup now accepts `BundleSessionOptions.flow` by
  resolving flow public ids to Product AWBC flow functions instead of rejecting
  every flow selector as `UnknownFlow`. Direct flow launches bind root arguments
  against the selected function signature, while ordinary entry launches keep
  the entry/function signature guard. This restores Agent observe `--flow`
  smoke coverage for authored image handle samples.
- Authored image object metadata now survives the bundle and Agent read-uri
  path: bundle image objects carry actions, typed params, proxies, and render
  frame indices; native Agent object image refs copy that metadata; object/layer
  image capture now runs before shared framebuffer fallback so object read-uri
  captures preserve the active textured-quad opacity and pixels.
- The `style::explicit_decl_id` hint now preserves declaration subpaths when it
  suggests compact spelling, for example `image @image.sample.pulse_sprite`
  suggests `image sample.pulse_sprite` rather than dropping the `sample`
  segment. `samples/image-animation.arcw` uses that compact spelling and checks
  cleanly without the previous hint.

### Verification

- `cargo test -p arcweft-lang-syntax --all-features explicit_entity_decl_id_prefers_compact_authoring_form -- --nocapture`
- `cargo run -p arcweft-cli --all-features --quiet -- check samples\image-animation.arcw`
- `cargo test -p arcweft-bundle --all-features bundle_image_objects_round_trip_as_typed_metadata -- --nocapture`
- `cargo test -p arcweft-cli --all-features bundle_image_objects_collect_declared_bounds_and_opacity -- --nocapture`
- `cargo test -p arcweft-runtime-driver --all-features product_awbc_session_flow_option -- --nocapture`
- `cargo test -p arcweft-cli --all-features --test check agent_observe_read_uri_preserves_animated_image_object_frame_metadata -- --nocapture`
- `cargo test -p arcweft-cli --all-features --test check agent_observe_reports_missing_scope_for_released_image_handle_object -- --nocapture`
- `cargo run -p arcweft-cli --all-features --quiet -- agent observe samples\image-animation.arcw --flow image_sprite_overlay --steps 2 --capture-time 0.15 --json`
- `cargo check -p arcweft-core -p arcweft-bundle -p arcweft-lang-syntax -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-core -p arcweft-bundle -p arcweft-lang-syntax -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native -p arcweft-cli --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit reported 0 errors and 146 warnings after this slice.

## 2026-07-07 Scoped Lifecycle Parity Evidence Refresh

After the retained View program substrate rename, the representative non-pinned
scoped-handle lifecycle and parity tests were rerun against current `main`.
They confirm that save/load, rollback, lexical cleanup stacks, Native Agent
capture filtering, Web prepared-frame filtering, and authored `image(...)` /
`view(...)` lifecycle cleanup still agree after the final View terminology
renames.

Validation:

- `cargo test -p arcweft-runtime-driver --all-features --test awbc_product_session -- --nocapture`
- `cargo test -p arcweft-cli --all-features --test check agent_observe_reports_authored_scroll_view_capture_and_release_filtering -- --nocapture`
- `cargo test -p arcweft-player-web --all-features --test parity web_runner_ -- --nocapture`
- `cargo test -p arcweft-player-web --all-features web_hidden -- --nocapture`
- `cargo test -p arcweft-cli --all-features --lib image_object_capture_scope_reports_missing_scope_diagnostic -- --nocapture`
- `cargo test -p arcweft-cli --all-features --test check agent_observe_reports_missing_scope_for_released_image_handle_object -- --nocapture`

No implementation drift was found in this refresh. Remaining non-goals are the
pinned exact PNG baseline promotion lane and the split scroll virtualization /
broader retained-content policy request.

## Remaining Work

- Product AWBC session save/load now persists the presentation handle table,
  rollback tombstones, compact-fiber cleanup stacks, and restored facade status
  through the typed `BundleSessionSnapshot` contract. Native player save/load
  UX now wraps those bytes with player-owned `InputControllerSnapshot` state;
  see
  `docs/implementation/seq-06.16.6.1-save-load-scoped-presentation-handles-2026-07-06.md`.
- Native Agent observe/capture now covers hidden image objects and hidden-only
  view scopes. Web-side parity now covers hidden view-owned text/button
  writeback, hit-test, focus, and stale activation rejection after the prepared
  frame drops those controls. Web-side prepared-frame reporting now also covers
  dropped image resources after image-handle lifecycle filtering. Native Agent
  diagnostics cover released image-object missing scopes both as a unit
  regression and as an authored `image(...)` handle integration smoke selected
  through final entry/goto dispatch. Web parity now also has an authored
  Product AWBC runner smoke for manual-live, released, and scoped-disposed
  image handles, and a Web Product AWBC runner smoke for View-owned text
  controls, action buttons, scroll regions, stale writeback rejection, stale
  activation rejection, and scoped cleanup. Native Agent parity now covers an
  authored Scroll view's live observed objects, view capture readback,
  scroll-clipped object omission, and released view/object missing-scope
  diagnostics. Broader parity still needs pinned GPU PNG readback promotion
  where exact visual baselines are required.
- `Scroll` now lowers from authored View syntax into compact UI scroll-region
  resources, runtime presentation snapshots, prepared-frame scroll regions, and
  native/web wheel routing with clamped `InputController` state. Text controls
  and action buttons authored inside `Scroll` now receive typed scroll
  ownership and prepare with offset, paint/text clipping, and visible
  hit/semantic bounds; runtime-control backdrop/filter/shadow plans now use the
  same visible bounds. Player-scene input snapshots can persist live scroll
  offsets, and native player session save/load now stores those snapshots in
  the saved file. Authored `Scroll(...)` named arguments now set stable scroll
  ids, viewport width/height, and typed overflow policy through resource,
  runtime, render, and input layers. Simple `Scroll` style rules can now supply
  viewport width/height and overflow defaults before local named arguments and
  modifiers override them. Authored `axis = .horizontal` now carries through
  resource, runtime, input snapshot, native/web wheel routing, renderer
  clipping, and compact codec contracts without old-shape compatibility
  defaults. Scroll-contained retained images and retained View text now have
  concrete runtime/render paths. Remaining scroll work is virtualization,
  larger retained-content policies, and adapter-parity broadening; that scope remains split to
  `docs/reviews/requests/2026-07-07-seq-06.16.6.2-scroll-axis-virtualization-retained-content.md`.
- The final UI syntax direction's await/pending builder integration no longer
  remains as a syntax/lowering gap: `AwaitView` parses into structured
  pending/ready/error/denied branches and lowers to `ViewProgramInstruction::Await`
  with branch spans. View-local input handle `let` bindings and ordinary `if`,
  `match`, and `for` View branching are covered by this cut.
- The active authoring, Agent observe/capture, layout capture scope paths, and
  retained `arcweft-ui` / Takumi substrate now use `view` terminology for owned
  scoped UI boundaries.
- Checked-in PNG visual baseline promotion remains outside this goal and is
  already covered by the pinned visual-golden requests such as
  `docs/reviews/requests/2026-07-04-seq-06.13e.1-inset-box-shadow-pinned-png-golden-promotion.md`
  and
  `docs/reviews/requests/2026-07-04-seq-06.13e.1.1-web-exact-png-readback-harness.md`.
