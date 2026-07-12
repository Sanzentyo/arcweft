# Unified Text / TextBox / View / Fx implementation status — 2026-07-12

Design source:

- external directive `arcweft-unified-text-textbox-view-implementation-directive.md`
- user-supplied final numeric/time/transform/target/bundle/save decisions
- [final repository design](../design/unified-text-textbox-view-fx-final-design-2026-07-12.md)

Baseline revision: `d934189ba1e414bfa23f7792658e69fd8c60d714`.

## Acceptance boundary

This goal is complete only when all display text converges on
`ResolvedTextDocument -> TextLayout -> PreparedTextItem ->
ViewPrimitive::Text -> SharedRenderer`, shared Fx executes for View and
RichText, TextBox renders through a persistent View mount, native capture uses
the shared prepared frame, and all legacy renderer paths listed by the source
directive are removed without compatibility shims.

## Current audit evidence

At the baseline revision:

- `ResolvedTextDocument`, `TextLayout`, `GlyphonTextEngine`, and
  `PreparedTextBatch` do not exist;
- `PreparedFrame` separately stores plain, selectable, and styled text;
- View `GlyphRun` only verifies a sidecar and is painted outside View order;
- `arcweft-text-layout` uses font-independent estimated advances;
- editable text owns a separate `FontSystem` and layout cache;
- native capture reprojects and relayouts `LineDisplayFrame`;
- native owns text effect/shader/motion registries and state;
- Fx sampler closures become labels and are dropped from RichText lowering;
- View Fx arguments are digests and `ApplyFx` is ignored at runtime;
- no Fx bundle section, shared evaluator, Fx save state, or common diagnostic
  path exists;
- presentation holds only one dialogue snapshot rather than a per-target
  TextBox store.

## Visual witness status

The existing checked-in vertical fixtures remain untouched. A candidate packet
was started under `target/unified-text-baseline-d934189b/`; its first release
build attempt exceeded the 600-second command limit before image generation.
The environment fingerprint was produced. A second run reused the completed
release build and generated the four vertical candidates, their observe/IMQ
records, and `vertical_goal_clear_smoke` successfully.

The first fixed-time effects capture attempt failed before rendering because
`samples/rich-text-effects-animation.arcw` contains unresolved
`@shader.source_glow` references at the baseline revision. A dedicated
`unified_text_effects_migration_baseline.arcw` fixture therefore carries the
same built-in/source-transform/time-dependent feature matrix using only
resolvable resources. A second two-line fixture keeps typewriter, stacked
motion, and vertical effects inside the visible legacy TextBox region.

The packet now contains fixed-time color captures and image statistics for the
effects matrix at 0, 0.125, 0.375, 1, 4, and 4.5 seconds, and for the
reveal/vertical matrix at 0, 0.5, 1, 4, 4.5, 20, and 20.5 seconds. `imq`
confirms time-dependent output:

| Comparison | PSNR | SSIM | MSE | MAE |
| --- | ---: | ---: | ---: | ---: |
| effects 4.0s vs 4.5s | 35.9841 dB | 0.973624 | 0.00025211 | 0.000506662 |
| reveal/vertical 20.0s vs 20.5s | 33.5661 dB | 0.954583 | 0.000439938 | 0.000879906 |

The migration preflight also exposed a baseline regression that predates this
change. Fresh normal-player candidates do not match the checked-in
native-rich-text vertical goldens and render the content through the horizontal
panel path. Representative metrics are:

| Fixture | PSNR | SSIM | MSE | MAE |
| --- | ---: | ---: | ---: | ---: |
| `vertical_tutr_golden` | 13.3255 dB | 0.004647 | 0.0465000 | 0.205113 |
| `vertical_lr_ruby_text_combine_golden` | 13.5004 dB | 0.003128 | 0.0446643 | 0.203239 |

The final shared path must restore the typed vertical/ruby/text-combine
behavior represented by the checked-in goldens; matching only the already
regressed baseline candidate is not sufficient.

`samples/rich-text-windows-fonts.arcw` was also excluded from image generation
because its current source has mismatched `[/strong]`/`[/em]` closes while a
font span is active. This source failure is independent of renderer output and
is not treated as passed evidence.

No checked-in golden may be overwritten automatically. Migration witnesses and
final shared-path goldens have distinct roles as specified by the final design.

## Cut tracking

- [x] Cut 1: migration witnesses and canonical resolved text document
- [x] Cut 2: typed Fx IR/evaluator/bundle/symbol/save contracts
- [ ] Cut 3: shaped shared text layout and glyphon engine
- [ ] Cut 4: prepared text batch and all ordinary producers
- [ ] Cut 5: RichText/reveal/shared Fx and native registry removal
- [x] Cut 6: direct View text painter order and executable per-mount View
- [ ] Cut 7: shared capture and prepared-layout Agent geometry
- [ ] Cut 8: persistent TextBox View and hardcoded dialogue removal
- [ ] Cut 9: final cleanup, parity, docs, and structural audit

### Implemented substrate after the design cut

The design and migration-witness cut is committed as `3d3167ba`. The current
implementation cut now contains:

- a borrowed, validated `ResolvedTextDocument` with canonical runs, ruby,
  closed style cascade, source origin/ranges, and source-record revisions;
- inherent stage/static-rich-text resolution without cloning a complete
  dialogue frame for each stage;
- a responsibility split of `arcweft-render-text` from a 1,768-line root to a
  47-line facade, with 30 focused tests and clippy passing;
- a responsibility split of `arcweft-text-layout` from a 2,458-line root to a
  35-line facade, retaining all vertical, UAX #50, JLREQ, ruby, and layout-phase
  effect behavior with 129 focused tests and clippy passing;
- typed syntax for path, grouped, glob, and alias imports instead of an opaque
  import-tree string at the HIR boundary;
- HIR retention of imports and canonical per-module ownership for functions,
  flows, and Agent controllers;
- one `CallableDeclarationId`/symbol table for ordinary and Fx callables,
  including alias/re-export identity preservation, ambiguity, private import,
  and visibility-widening diagnostics; the project compiler retains this
  table for subsequent sema and runtime-plan lowering;
- finite typed Fx values (`f32`, px lengths, radian angles, seconds, colors,
  vectors, and the closed `Transform2D` contract), typed static graph
  properties, validated sampler bytecode, a single Sans I/O evaluator, shared
  operation budgets, deterministic logical time/ordinal/seed behavior, typed
  renderer capabilities, transactional resolved plans, and typed diagnostics;
- runtime-plan lowering of Fx closures to executable sampler instructions,
  including context time, logical ordinal, golden-angle phase, pure arithmetic,
  trigonometry, typed parameter slots, and `Transform2D` construction without
  source-string or zero fallbacks;
- a first-class independently checksummed `FxDefinitions` AWFB section holding
  canonical `FxId`, ABI/semantic hashes, parameter schemas, typed graphs,
  samplers, and renderer interfaces; the unpublished bundle contract was
  replaced directly without an alias reader, migration shim, or schema-version
  bump;
- bundle compilation now resolves and attaches the package Fx inventory rather
  than dropping definitions after runtime-plan lowering;
- runtime-driver-owned live Fx state with deterministic logical clock advance,
  stable activation time and seed, reactive parameter-only updates, bounded
  provider/child state, and canonical instance ordering;
- atomic Product AWBC session save/load of the complete Fx instance table,
  including definition existence, ABI, parameter type/count, activation time,
  deterministic seed, child path, and provider state validation before any
  session field is changed;
- the common `FxDiagnostic` contract is carried directly in Web observations
  and projected with stable code/severity/Fx identity into Agent observations.

The shaped layout contract, real project-font glyphon shaper, canonical
`ResolvedTextDocument` consumer, and first product `PreparedTextBatch` path now
exist. RichText and stateless/legacy text producers, View Fx
evaluator/application mounting, direct View painter order, capture convergence,
and TextBox convergence remain open. The overall goal therefore remains active.

### Shaped-layout implementation slice

The first half of Cut 3 is now implemented without enabling a parallel legacy
fallback:

- `GlyphonTextEngine` starts from an empty `fontdb`, registers only the exact
  ordered project font bytes, uses an empty platform-fallback policy, and owns
  one `FontSystem`, `SwashCache`, bounded shaped-run cache, stable
  `FontFaceId` mapping, and renderer-local raster-key preparation boundary;
- the shape cache key contains exact font inventory/features, engine and run
  locale, source text, family stack, size, line height, weight, slant,
  letter/word spacing, writing mode, and inline direction while excluding
  source offset and paint-only color; cached source ranges are rebased on
  return;
- cosmic-text shaped clusters now provide ligatures, combining marks, bidi
  visual order, fallback faces, actual advances, and Swash raster ink bounds;
  missing glyphs, failed rasterization, invalid metrics, invalid cache flags,
  and negative word-spacing advances are structured errors rather than clamps
  or zero fallbacks;
- `layout_document` uses actual shaped cluster metrics for horizontal and
  vertical placement, hard-line tracking, visual line/column identity,
  text-combine scaling, sideways Latin rotation geometry, and JLREQ
  line-head/line-end protection at overflow;
- `TextLayoutGlyph` now separates raster `ink_bounds` from logical
  `layout_bounds` used by hit/selection geometry; ruby owns shaped glyph keys,
  origins, bounds, and collision-adjusted tracks inside `TextLayout` instead
  of requiring a renderer-native second layout;
- the layout hash includes the exact font inventory, revision, constraints,
  resolved layout style, final body geometry, ruby geometry, orientations,
  scales, and stable glyph keys while excluding paint color;
- real bundled Japanese/emoji/Latin project-font tests cover deterministic
  hash parity, fallback, ligatures, combining marks, explicit RTL, hard breaks,
  CJK, vertical text-combine, sideways Latin, and shaped ruby.

Cut 3 remains open because `LaidOutText`, `layout_frame`, and the old
`LaidOutText -> GlyphArea` adapter still have live renderer/native call sites.
They will be removed directly when those call sites move to
`PreparedTextBatch`; this slice does not add a compatibility wrapper.

### Prepared-batch ordinary-text slice

The first product path for Cut 4 is implemented:

- `PreparedTextBatch` owns frame-local, painter-ordered `PreparedTextItem`
  values addressed by `PreparedTextId`; each item carries the one canonical
  `TextLayout`, resolved raster keys, paint plan, interaction geometry, clip,
  and physical raster scale;
- paint retains per-glyph visibility, color, opacity, finite affine transform,
  glyph masks, offscreen operations, and post-process operations. Invalid
  phase/opacity/clip data fails through `PreparedTextError` rather than being
  ignored or replaced by a fallback;
- body and ruby glyphs are submitted together from the same layout. Vertical
  quarter-turn orientation can be composed with the shared post-layout affine
  transform without reshaping or changing the layout hash;
- `SharedFramePlanContext` converts mapped ordinary `RenderTextBlock` inputs
  to `ResolvedTextDocument -> TextLayout -> PreparedTextItem`, preserves source
  selection and character geometry, and resolves scale-specific glyph cache
  keys once per prepared frame;
- long-lived native/Web product planners finalize after viewport mapping, so
  project-font registration produces an empty legacy ordinary-text queue and a
  populated prepared batch at the final viewport coordinate system. A direct
  player-scene test covers this registered-font route;
- `SharedRenderer` paints prepared glyph areas directly and uses the same
  prepared ranges for normal and filtered runtime-control passes; the ignored
  adapter-dependent GPU smoke was run explicitly and confirmed that the batch
  changes captured pixels without renderer-side shaping;
- selection adapters and Web observation counts now derive from the prepared
  item interaction plan; shape-cache hit/miss/entry counts are exposed through
  the shared planner statistics;
- renderer unit tests were moved to the renderer responsibility submodule when
  the structural gate detected the production file above 2,500 physical LOC.
  The production file is now 2,447 LOC and the final audit has no errors.

Cut 4 remains open because the no-font stateless facade, public legacy
`PreparedFrame::text`, and styled-paragraph/RichText producers still have live
call sites. They are intentionally visible migration inputs, not a second
fallback inside the new prepared contract, and must be deleted as the remaining
producers converge.

### Canonical RichText dialogue-preparation slice

The product dialogue path now uses the same prepared-text contract:

- a display stage resolves directly to a borrowed `ResolvedTextDocument`, and
  `[clear]` projects the remaining document back to the `TextBox` origin while
  preserving clipped run/source ranges and contained ruby annotations;
- vertical body text, shaped ruby, reveal visibility, selection geometry, and
  raster keys are produced by one `TextLayout` and one `PreparedTextItem`;
- reveal changes only glyph paint visibility. It no longer changes shaping or
  the layout hash, and reduced motion completes reveal immediately;
- static transforms and the built-in wave, shake, jitter, arc, spin, and pulse
  effects are composed after layout. Their phase is based on
  `TextLayoutGlyph::logical_ordinal`, never a UTF-8 byte offset, and shared
  deterministic noise uses the presentation Fx seed/time bucket contract;
- the long-lived product planner consumes the legacy mapped paragraph only as
  a placement input, emits the prepared item, clears the old styled-paragraph
  queue, and preserves painter order after ordinary prepared text.

### Typed RichText Fx lifecycle and shared evaluation slice

The compiler/runtime/renderer boundary no longer serializes an Fx graph into a
legacy effect label:

- `[fx call(...)]` now retains one bounded `FxApplication` containing the
  canonical `FxId`, typed parameter values, authored ordinal, and source range;
  sampler programs and nested graphs remain in the bundle definition;
- runtime reconciliation derives a stage-independent instance from the
  dialogue occurrence, line identity, and authored ordinal. A retained line
  keeps activation logical time and deterministic seed while only parameter
  slots refresh, and removed occurrences release their instances;
- the shared `FxGraphEvaluator` evaluates static values, parameter slots,
  samplers, conditional branches, and authored stacks under one per-instance
  frame budget. An application either commits its complete resolved plan or
  emits a typed diagnostic with no partial operation;
- resolved operation values now retain resources, selectors, strings, lists,
  and records as closed typed values rather than dropping everything except
  runtime scalars;
- canonical dialogue preparation applies `Fx.text` before shaping and typed
  transform/color/mask operations after layout using logical glyph ordinals.
  Frame-time diagnostics are exposed in prepared frames, Web frame
  observations, and Agent observations;
- a source-to-HIR-to-runtime-plan-to-bundle-to-session-to-prepared-glyph test
  proves that one authored sampler keeps its live instance across steps and
  changes the canonical glyph paint without changing renderer arithmetic.

### Shared Fx compositor slice

Prepared paint is now executable rather than metadata-only:

- presentation owns closed finite `ResolvedFxGlyphPass`, `ResolvedFxMask`,
  `ResolvedFxOffscreenPass`, and `ResolvedFxPostProcess` contracts plus one
  deterministic `FxRenderResourceTable`; backends no longer receive raw graph
  operations or callback-shaped shader data;
- the shared table resolves `soft_glow`, `warm_glow`, `screen_tint`, and the
  authored `@shader.source_glow` program identically for native, Web, headless,
  and offscreen rendering. Unknown resources and invalid uniform schemas emit
  typed diagnostics rather than becoming no-ops;
- glyphon emits additional glow/color passes before the main glyph and applies
  constant glyph-mask coverage to the same body/ruby glyph keys, orientations,
  affine transforms, opacity, and reveal visibility;
- `SharedRenderer` isolates only effectful prepared items, preserves item
  painter order, executes blur/brightness/contrast/saturation through the
  existing View filter machinery, runs tint/displacement/sparkle post-process
  programs in the shared compositor shader, and composites back over the
  existing scene target;
- legacy RichText descriptors are normalized once in the shared preparation
  module. Wave/shake/jitter/arc/spin/pulse, sparkle, the two retained motion
  programs, typewriter, legacy shader references, and their post-process forms
  no longer require native arithmetic to reach the normal player path;
- the source-to-product parity fixture now compiles both glyph and post-process
  `Fx.shader` nodes, retains one live Fx instance, and proves the resulting
  passes are present in the canonical prepared item;
- renderer submission was split into a 171-line responsibility module after
  the main renderer crossed the 2,500-LOC structural error threshold. The
  renderer returned to 2,370 LOC and the audit reports no error-level files.

Cut 5 remains open because `arcweft-render-native` still contains the old
effect/shader/motion registries and its legacy capture renderer still consumes
`LineDisplayFrame`. Those APIs will be deleted with shared prepared-frame
capture rather than retained as a fallback. Runtime-host injection of
additional typed render-resource programs also remains to be connected to the
shared table before native provider loading can replace the old callback API.

### Direct View prepared-text slice

The renderer-facing half of Cut 6 now uses the canonical batch directly:

- `ViewPrimitive::Text(ViewTextPrimitive { text: PreparedTextId })` replaces
  `GlyphRun`, Selection, Caret, and CompositionUnderline variants. The
  `PreparedViewGlyphRunHandoff` sidecar and its renderer resource table are
  deleted rather than retained as compatibility vocabulary;
- `ViewCompositor` supplies a grouped `ViewDirectRenderFrame` and invokes a
  dedicated `ViewTextRenderer` exactly where the Text primitive occurs. The
  shared renderer resolves the frame-local ID, rejects a missing item with
  `ViewCompositorError::MissingPreparedText`, and submits the already shaped
  glyphs without a View-specific layout or string registry;
- `PreparedTextAffine` validates finite context matrices and [0, 1] opacity,
  composes translation/scale/rotation with body, sideways, text-combine, and
  ruby glyph transforms, and applies opacity once to glyph and Fx-pass alpha;
- item clip and View clip are intersected in the active target coordinate
  space. Selection rectangles render before glyphs, while caret and IME
  composition underlines render afterward from the same interaction plan;
- offscreen groups retain the parent coordinate space in a bounded pooled
  target, preventing a bounds-sized texture from being stretched over the
  parent. Device scale is retained for filters and glyph rasterization;
- every prepared ID consumed by a View scene is excluded from the later
  ordinary prepared range, while repeated uses inside View painter order remain
  legal; and
- the Takumi text bridge now carries `PreparedTextId` directly and emits Text
  primitives rather than a renderer-local glyph-run descriptor.

Local WGPU readback proves that Text changes pixels at its direct painter
position, a later opaque primitive covers it exactly with no late duplicate
submission, and transform, rectangular clip, context opacity, group opacity,
and an offscreen group all remain effective. A separate readback proves that a
missing ID returns the typed compositor failure rather than a no-op.

Cut 6 remains open: the bundle `ViewRuntimeTextBlock` snapshot path still has to
be replaced by an executable persistent per-mount View evaluator that resolves
plain/localized/RichText/DisplayFrame sources before preparation. This slice
does not misclassify that remaining producer work as complete.

### Persistent View value-evaluation substrate

The first evaluator half of Cut 6 is now an executable Sans I/O contract in
`arcweft-view`:

- `ViewValueProgram` is a distinct validated owner over the common typed value
  instruction model and View-specific instruction, constant, stack, and slot
  limits. Deserialization revalidates the complete program instead of trusting
  serialized derived state;
- `ViewValueProgramInventory` rejects duplicate IDs and requires one common
  parameter/state input schema for all programs owned by a mounted View;
- `ViewMountId` is now the general occurrence identity rather than a type owned
  only by list virtualization, and the monotonic allocator is shared as an
  explicit View boundary type;
- each `ViewMountState` owns parameter/state values, monotonic slot revisions,
  and program results. A cached result is reused only when every slot actually
  consumed by that program has the same revision, so unrelated dirty slots do
  not trigger evaluation;
- mount snapshots retain program identity, state-schema hash, typed values, and
  revisions. Restore validates identity, schema, counts, and every value type
  before constructing replacement state; caches are deliberately rebuilt; and
- focused tests cover dependency-level invalidation, independent mounts,
  snapshot validation, invalid serialized stack programs, and preservation of
  the existing virtual-list allocator semantics after moving mount identity.

This substrate does not by itself complete Cut 6. The next slice replaces the
View bundle's digest-only references with actual program records, then mounts
and evaluates that inventory in the runtime driver before emitting prepared
text IDs in View painter order.

### Executable View bundle inventory

The bundle/compiler half of the persistent evaluator is now implemented:

- `condition_schema`, `value_schema`, `source_schema`, `key_schema`,
  `props_schema`, and the other digest-only expression fields are removed.
  Instructions reference validated `ViewValueProgramId` records directly; no
  dual reader, alias field, or format-version shim was added;
- View source lowering compiles finite literals, typed state/local projections,
  boolean and arithmetic operators, comparisons, explicit scalar intrinsics,
  match conditions, keyed-repeat count/key values, await status projections,
  nested View arguments, and reactive Fx arguments into the common closed
  instruction model. Fx argument expectations come from the compiled
  `FxDefinition` parameter schema rather than guessed names or raw tokens;
- decimal/unit conversion happens once during compilation. Invalid numbers,
  unsupported units, projection type conflicts, unsupported expression shapes,
  and value-program limit failures are structured compile errors; there is no
  zero or debug-string fallback;
- the View program section stores typed external input-slot sources and rejects
  incomplete coverage, duplicate slots, invalid projection paths, missing
  program references, wrong condition/repeat result types, invalid stack
  programs, and out-of-range control-flow spans during canonical decode;
- text literals remain static graph data, while reactive plain text is retained
  as a typed state/local text projection. Strings are not smuggled through the
  numeric value stack;
- await branches now retain both relative start and length, so authored branch
  order cannot make runtime selection ambiguous; and
- merging authored and DSL View resources rebases program IDs, parameter/state
  slots, instruction references, child-span indices, and instruction ranges.
  Independent resources therefore cannot collide when packed into one initial
  View section.

Direct tests execute the compiled `if`, repeat-count, and repeat-key programs
through `ViewMountState`, prove dirty repeat ordinals change the key result,
round-trip canonical bundle bytes, and reject missing/wrongly typed program
references. The compiler module was split into a 675-line expression owner and
a 272-line literal-conversion owner rather than leaving a new 1,000-line mixed
responsibility file.

Cut 6 remains open until the runtime driver owns these program/mount records,
persists them in save state, resolves projected text, and emits prepared text
IDs instead of `ViewRuntimeTextBlock` snapshots.

### Executable per-mount View runtime and session projection slice

The runtime-driver half of the retained evaluator is now implemented:

- every live presentation handle gets an independent root occurrence; nested
  calls and keyed repeats use a stable structural path, while one shared
  monotonic allocator issues collision-free `ViewMountId` values across root
  and child occurrences;
- the evaluator executes branch, keyed repeat, await, nested call, local bind,
  text/image emission, and typed View Fx application under bounded instruction
  and value-program budgets. Missing slots, duplicate repeat keys, invalid await
  discriminants, recursion, unsupported text values, and malformed spans produce
  mount/handle/instruction diagnostics rather than placeholder execution;
- value conversion accepts only exact finite typed values. In particular an
  arbitrary-width runtime integer cannot silently enter an `I32` slot, and
  Length/Angle/Seconds/Color/Vec2/Transform2D cross the boundary through their
  closed record contracts;
- logical time advances from the deterministic session clock, per-mount sample
  time starts at activation, reduce-motion freezes sampler time, and context
  cache keys include time/ordinal/seed only when a program actually consumes
  context;
- evaluated output is mount-scoped into existing image/control/button/scroll/
  surface/text/focus resources. Image and scroll lowering now retain concrete
  target IDs, and duplicate handles may legally instantiate the same reusable
  View definition;
- scoped TextInput write-back updates only its occurrence and survives the next
  projection frame and save/load; public/masked/secret View text remains typed
  through observation redaction;
- View Fx applications reconcile into the same session-owned Fx runtime used by
  RichText, retaining instance identity while reactive parameters change; and
- session save/load atomically validates View program identity, allocator cursor,
  occurrence/path/mount identity, activation time, seed, typed slots/revisions,
  runtime parameters, and consistency between the saved presentation frame and
  retained mount table. Content-only hot swap restores the same contract only
  after existing virtualization compatibility checks pass.

Focused tests cover missing input without placeholder fallback, two simultaneous
mounts of one definition, reactive branch changes, independent scoped TextInput
state, nested mount snapshot restore, fresh post-restore allocation, duplicate
repeat keys, logical-time/reduced-motion behavior, exact `I32` width, observation
redaction, and atomic rejection of a tampered presentation/mount identity.

Cut 6 remains open only for the renderer-preparation half: localized,
RichText-document, and display-frame View sources must be resolved directly into
`PreparedTextBatch`, and the temporary `ViewRuntimeTextBlock` projection must be
removed rather than retained as a plain-text adapter.

Validation for this slice at Jujutsu working change `lsunwnmw`:

```bash
cargo test -p arcweft-runtime-driver --all-targets --no-fail-fast
cargo test -p arcweft-bundle --all-targets --no-fail-fast
cargo test -p arcweft-cli app::bundle::tests --lib -- --nocapture
cargo clippy -p arcweft-view -p arcweft-bundle \
  -p arcweft-runtime-driver -p arcweft-cli --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-runtime-session-2026-07-12
```

All commands pass. The runtime-driver suite passes 40 unit tests, 16 Product
AWBC session tests, 37 session integration tests, and 5 direct View-runtime
tests. The bundle suite passes 80 unit tests and all integration suites; the
focused CLI bundle suite passes 41 tests; and `just test-fast` passes 422 tests.
The first CLI invocation exhausted its 300-second process budget immediately
after compiling and reporting all 41 tests passed; the warm rerun exited 0 in
under two seconds.

The structural audit records 1,257 Rust files / 631,790 physical Rust LOC, 0
errors, and 141 warnings, with no workspace dependency-edge change. New
production responsibilities are 692 LOC for the runtime/snapshot owner, 1,192
LOC for control-flow evaluation, 442 LOC for exact runtime/Fx value conversion,
360 LOC for mount-scoped resource projection, and 270 LOC combined for text and
evaluation support modules. The existing session orchestrator is 2,404 LOC and
remains a warning-level hotspot below the 2,500-LOC error threshold; its View
algorithm is already isolated in the modules above, while the remaining file
owns the pre-existing session lifecycle, hot swap, tasks, input, and save
transaction orchestration.

### Typed View text preparation and exact mounted painter closure

Cut 6 is now complete. This slice supersedes the earlier interim notes that
still named `ViewRuntimeTextBlock` as the remaining adapter:

- `ViewTextResource` now owns exact typed localized, rich-document, and
  display-frame stores. Source records address stable IDs, localized lookup is
  exact on key/locale, and display stage selection is validated. Missing
  localized text, document, frame, or stage produces `VIEW014` through
  `VIEW017`; no empty/debug/plain fallback is emitted;
- bundle compilation may hydrate only a locale-unspecified source from the
  canonical `LineDisplayCatalog` entry with the same text key. Explicit locale
  records remain exact authored data and never fall through to another locale;
- `BundleViewTextValue` carries the actual typed `RichTextDocument` or
  `LineDisplayFrame` to the player boundary. The shared resolver constructs one
  `ResolvedTextDocument`, then `SharedFramePlanContext` appends it directly to
  the frame's canonical `PreparedTextBatch`;
- vertical-rl, ruby, text-combine, run-source provenance, selection geometry,
  scroll clipping, and resolved runtime style survive the same preparation
  path. Focused player tests assert vertical body/ruby layout and exact
  localized/display-stage visible text rather than a placeholder;
- `ViewRuntimeTextBlock` and its bundle, runtime snapshot, session projection,
  presentation-handle filtering, and player adapter fields were deleted
  directly. The stateless player registers the bundle default fonts before
  one-shot preparation instead of leaving a legacy pending text block;
- mount paint output now includes nested-mount insertion points. The player
  expands a child at that exact parent slot, so parent-before, child, and
  parent-after primitives keep evaluator order even though mount state records
  remain canonically sorted for save/observation;
- View-owned images move out of the renderer's ordinary background image pass
  into the same prepared View scene. Their decoded frame is transferred to the
  scene resource table and their crop UV, affine transform, and opacity are
  retained. This prevents duplicate painting and makes Element/Text/Image/
  nested-View order one renderer contract; and
- the player-scene crate now depends directly on the low-level render-text and
  text-layout contracts it consumes. No higher-level or platform dependency
  was introduced.

Focused tests cover all typed stores and diagnostics, codec canonicalization,
CLI default-localization hydration, runtime redaction, exact nested mount paint
IR, recursive parent/child painter expansion, View image resource transfer and
crop/transform preservation, prepared-only plain text, selectable scroll
geometry, and vertical ruby preparation.

Validation at Jujutsu change `xuxwrolx`:

```bash
cargo test -p arcweft-bundle --all-targets --no-fail-fast
cargo test -p arcweft-runtime-driver --all-targets --no-fail-fast
cargo test -p arcweft-player-scene --all-targets --no-fail-fast
cargo test -p arcweft-render-text --all-targets --no-fail-fast
cargo test -p arcweft-render-wgpu --all-targets --no-fail-fast
cargo test -p arcweft-cli app::bundle::tests --lib -- --nocapture
cargo test -p arcweft-player-web --test parity --no-fail-fast
cargo test -p arcweft-view --all-targets --no-fail-fast
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-prepared-sources-2026-07-12
git diff --check
```

All final commands pass. The runtime-driver all-target route passes 98 tests,
the player-scene route passes 85, the CLI bundle module passes 42, Web parity
passes 7, and `just test-fast` passes 422. Render-text and render-wgpu pass all
normal tests; the adapter-pinned Tier 2 WGPU/exact-PNG tests remain ignored by
the normal gate as required by repository policy.

The first cold aggregate invocation of the five largest changed crates reached
its 244-second command limit while compiling, with no test result available;
each crate was then run separately to completion. Web parity initially exposed
that its hand-built View-control fixture mounted `view.WebPanel` without an
executable definition. The fixture now carries the typed definition and exact
target emissions, and asserts the resulting mount-scoped IDs; the focused and
complete parity reruns pass.

The final structural audit records 1,258 Rust files / 633,404 physical Rust
LOC, 0 errors, and 142 warnings. `frame/view_text.rs` is a 344-LOC
responsibility module and `frame/surfaces.rs` is 730 LOC including 202 embedded
test LOC. The full changed-file, largest-file, embedded-test, responsibility,
fan-in, and fan-out review is recorded in the linked audit directory.

## Required validation

Focused tests follow the repository test policy. Reviewable cuts additionally
run workspace check/clippy, formatting, diff checks, and a written structural
audit. Renderer/capture completion runs the relevant Tier 2 visual and WebGPU
parity gates. Any environment failure is recorded with the exact command and
passed focused evidence; it is not reclassified as completion.

Initial structural audit:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-final-design-2026-07-12
```

Result: 1,191 Rust files / 604,079 physical Rust LOC, 0 errors, 149
warnings. Directly relevant baseline hotspots include the oversized facade or
production files in `arcweft-render-text`, `arcweft-text-layout`,
`arcweft-glyphon`, `arcweft-render-wgpu`, `arcweft-render-native`, the View
resource codec, runtime display/session, and native Agent capture. The
implementation cuts must split those files by responsibility while removing
the duplicated paths; adding the new model into the existing monoliths is not
an acceptable final structure.

Cut 2 validation at Jujutsu change `rrnvupruqzsu`:

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-fx-contracts-2026-07-12
```

All commands pass. `just test-workspace` initially found one stale
`mod game::opening` spelling after 500/501 semantic fixtures. The final grammar
uses dotted module paths, so the fixture is now `mod game.opening`; the parser
rejects `::` through a structured diagnostic, and the module-tail lint reads
the typed path instead of splitting source text. The focused parser test, the
501-fixture semantic suite, and the complete workspace rerun pass. Existing
adapter/device-dependent Tier 2 visual tests remain ignored by the normal
workspace gate as required by the test policy. The final Cut 2 structural audit
records 1,231 Rust files / 615,563 physical Rust LOC, 0 errors, and 144
warnings in the linked audit directory.

Shaped-layout slice validation at Jujutsu change `qslpmxxq`:

```bash
cargo test -p arcweft-text-layout --all-targets
cargo test -p arcweft-glyphon --all-targets
cargo clippy -p arcweft-text-layout -p arcweft-glyphon --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-shaped-layout-2026-07-12
```

All commands pass. `just test-fast` passes 422 focused tests across its five
suites. The structural audit records 1,235 Rust files / 617,860 physical Rust
LOC, 0 errors, and 144 warnings. New responsibilities are split across the
787-line document placer, 470-line ruby placer, and 221-line layout hash
module. The 1,150-line glyphon engine remains below the production warning
threshold; its cache, project-font inventory, shaping, and raster-key
responsibilities remain internal to that one engine and will be reconsidered
when the legacy adapter is removed from the crate facade.

Prepared-batch slice validation at Jujutsu change `oyvzyvzo`:

```bash
cargo test -p arcweft-glyphon --all-targets
cargo test -p arcweft-render-wgpu --test prepared_text
cargo test -p arcweft-player-scene --all-targets
cargo test -p arcweft-render-wgpu --test prepared_text \
  prepared_batch_renders_without_renderer_side_shaping -- --ignored --exact
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-prepared-batch-2026-07-12
```

All commands pass. `just test-fast` passes the same 422 focused tests, and the
explicit WebGPU smoke passes on the local adapter. The structural audit records
1,239 Rust files / 619,203 physical Rust LOC, 0 errors, and 143 warnings. The
new production responsibility modules are `prepared_text.rs` in
`arcweft-glyphon` (651 LOC) and the ordinary-block lowering module in
`arcweft-render-wgpu` (123 LOC). Existing renderer and geometry warnings remain
tracked for removal/splitting as the legacy paths are deleted.

Canonical RichText preparation validation at Jujutsu change `qmmymwws`:

```bash
cargo test -p arcweft-render-text --test resolved_document \
  document_projection_rebases_runs_and_ruby_without_cloning_text -- --exact
cargo test -p arcweft-render-wgpu --lib \
  geometry::dialogue_prepared::tests -- --nocapture
cargo test -p arcweft-player-scene --all-targets
cargo clippy -p arcweft-render-text -p arcweft-presentation \
  -p arcweft-render-wgpu -p arcweft-player-scene --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-rich-dialogue-prepared-2026-07-12
```

All commands pass. The focused dialogue tests cover vertical ruby, paint-only
reveal with a stable layout hash, `[clear]` projection, and logical-ordinal
wave sampling. The structural audit records 1,240 Rust files / 620,106
physical Rust LOC, 0 errors, and 143 warnings. The new dialogue-preparation
module is 720 LOC, inside the ordinary responsibility-module target range.

Typed RichText Fx validation at Jujutsu change `twnzxlzw`:

```bash
cargo test -p arcweft-presentation graph_evaluator -- --nocapture
cargo test -p arcweft-runtime-plan render_text::fx -- --nocapture
cargo test -p arcweft-render-wgpu --lib \
  geometry::dialogue_prepared::tests -- --nocapture
cargo test -p arcweft-runtime-driver --all-targets
cargo test -p arcweft-player-web --test parity \
  authored_rich_text_fx_retains_one_runtime_instance_and_uses_shared_evaluator \
  -- --exact --nocapture
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-typed-richtext-fx-2026-07-12
```

All commands pass. `just test-fast` passes 422 tests. The structural audit
records 1,243 Rust files / 621,309 physical Rust LOC, 0 errors, and 143
warnings. The dialogue preparation implementation is 974 LOC and its 316 LOC
test suite is a child responsibility module; the new application and graph
evaluator modules are 180 and 435 LOC respectively.

Shared Fx compositor validation at the succeeding working change:

```bash
cargo test -p arcweft-presentation -p arcweft-glyphon \
  -p arcweft-render-wgpu --all-targets
cargo test -p arcweft-render-wgpu --test prepared_text \
  prepared_batch_renders_without_renderer_side_shaping -- --ignored --exact --nocapture
cargo test -p arcweft-player-web --test parity \
  authored_rich_text_fx_retains_one_runtime_instance_and_uses_shared_evaluator \
  -- --exact --nocapture
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-shared-fx-compositor-2026-07-12
```

All commands pass. The explicit local-adapter smoke confirms that glyph pass,
mask, blur, and post-process data changes captured pixels through the actual
WGPU shader path. `just test-fast` passes 422 tests. The structural audit
records 1,246 Rust files / 623,404 physical Rust LOC, 0 errors, and 143
warnings. The new shared resource module is 697 LOC, legacy descriptor
normalization is 1,039 LOC, canonical dialogue preparation is 784 LOC, and the
prepared renderer responsibility module is 171 LOC.

Direct View prepared-text validation at Jujutsu change `rwoyplsl`:

```bash
cargo test -p arcweft-glyphon -p arcweft-render-wgpu \
  -p arcweft-takumi-adapter --all-targets --no-fail-fast
cargo test -p arcweft-render-wgpu --test prepared_text \
  view_text_renders_at_primitive_position_without_late_duplicate_submission \
  -- --ignored --exact --nocapture
cargo test -p arcweft-render-wgpu --test prepared_text \
  view_text_obeys_transform_clip_opacity_inside_offscreen_group \
  -- --ignored --exact --nocapture
cargo test -p arcweft-render-wgpu --test prepared_text \
  missing_view_text_id_is_a_typed_compositor_failure \
  -- --ignored --exact --nocapture
cargo test -p arcweft-render-wgpu --test prepared_text \
  view_text_interaction_paints_selection_before_glyphs_and_ime_after \
  -- --ignored --exact --nocapture
cargo test -p arcweft-render-wgpu --test view_box_shadow_gpu_smoke \
  per_corner_outer_and_elliptical_inset_shadow_cards_execute_gpu_compositor_path \
  -- --ignored --exact --nocapture
cargo clippy -p arcweft-glyphon -p arcweft-render-wgpu \
  -p arcweft-takumi-adapter --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-direct-view-text-2026-07-12
```

The focused suites, all four direct-text local-adapter WGPU readbacks, the
offscreen-group shadow smoke, workspace all-features check/clippy, and
`just test-fast` pass; the fast route reports 422 tests. The structural audit
records 1,247 Rust files / 624,053 physical Rust LOC, 0
errors, and 143 warnings. Changed production sizes are 816 LOC for canonical
prepared-text data/submission, 250 LOC for the new View text callback, 263 LOC
for shared prepared submission, 1,049 LOC for direct primitives, 1,557 LOC for
the compositor, and 2,403 LOC for the still-tracked legacy-bearing renderer.
No workspace dependency edge changed. The compositor and renderer remain
warning-level ownership hotspots; their responsibilities are split between
direct primitives, prepared text, View text, pass planning, and execution, and
will shrink further as the old non-batch renderer paths are deleted.

Persistent View value-runtime validation at the succeeding working change:

```bash
cargo test -p arcweft-view --all-targets
cargo clippy -p arcweft-view --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-value-runtime-2026-07-12
```

All commands pass. The focused suite includes 43 unit tests plus the retained
interaction, motion, and eight virtual-list integration tests. The structural
audit records 1,248 Rust files / 624,856 physical Rust LOC, 0 errors, and 143
warnings. The new value-runtime responsibility module is 736 LOC, within the
preferred ordinary-module range. No workspace dependency edge changed.

Executable View bundle-inventory validation at the succeeding working change:

```bash
cargo test -p arcweft-view --all-targets
cargo test -p arcweft-bundle --all-targets --no-fail-fast
cargo test -p arcweft-cli app::bundle::tests --lib
cargo clippy -p arcweft-view -p arcweft-bundle -p arcweft-cli \
  --all-targets -- -D warnings
cargo test -p arcweft-lang-syntax -p arcweft-lang-sema --all-targets
cargo test -p arcweft-runtime-plan render_text::fx --lib
cargo test -p arcweft-lsp \
  hover_includes_expanded_fx_style_contributions --lib -- --nocapture
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 just test-workspace
cargo clippy -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-view \
  -p arcweft-bundle -p arcweft-cli --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-executable-view-bundle-2026-07-12
```

All commands pass. The bundle suite passes 80 unit tests and every integration
suite, the focused CLI bundle suite passes 40 tests, and the View suite passes
43 unit tests plus interaction, motion, and virtualization integration tests.
The workspace fast path initially exposed two genuine regression fixtures. A
positive semantic fixture still used `mod game::routes::opening`; it now uses
the canonical dotted `mod game.routes.opening`, while the syntax suite retains
its explicit rejection test for `mod game::opening`. The typed RichText Fx
migration had also retained only Fx identity in LSP cascade provenance. Fx text
properties are now resolved through the one shared evaluator at deterministic
zero logical time with reduced motion, restoring the observable
`rich_text.text.color` contribution without adding a second arithmetic path.
The complete workspace rerun passes after both fixes.

The structural audit records 1,250 Rust files / 626,656 physical Rust LOC, 0
errors, and 143 warnings. Moving executable-inventory merge/rebasing into its
225-line responsibility module reduced `bundle.rs` from the 2,513-LOC error
state found by the first audit pass to 2,289 LOC. The expression compiler is
675 LOC and exact literal conversion is 272 LOC. `target/` was removed once
after mixed prior feature/profile builds had accumulated 288.58 GiB; the final
workspace test used the same recipe feature set with incremental compilation
and test debug symbols disabled solely to bound disposable artifact size.

Executable View definition-contract closure at the succeeding working change:

- `ViewProgramResource` now owns one `ViewDefinitionResource` per reachable
  View declaration. Each record carries its exact instruction span, ordered
  parameter schema, executable scalar default, and mount-state schema hash.
- `CallView` addresses the target definition directly. Its arguments are
  canonicalized by parameter ordinal and validated for target existence,
  duplicate/missing bindings, authored-name agreement, program existence, and
  scalar result type.
- scalar View parameters now occupy the common value inventory's typed
  `parameter` schema and retain their exact definition-scoped slot on the
  parameter record. Local and repeat-ordinal state inputs also include the
  owning definition ID. Two definitions may therefore use the same parameter
  or local name with different types without aliasing; codec validation proves
  the parameter record, input source, namespace, slot, and type agree.
- the provisional single `root_view`, unowned `child_spans`, digest-only state
  schema list, and `CallView.child_span` were replaced directly; this is an
  unpublished internal format, so no alias, dual reader, migration shim, or
  version bump was added;
- compact nested calls such as `Child(value = 3)` now produce the existing
  typed `ViewCall` AST. Plain and relative calls are module-scoped, so under
  `mod game.opening` the call resolves to `view.game.opening.Child`;
- bundle inclusion starts from flow-mounted roots and computes the transitive
  nested-View closure. Cycles converge by ID set, while unknown definitions are
  structured lowering failures.

Focused validation covers dotted module scoping, parser preservation, nested
reachability, exact non-overlapping definition coverage, typed/default
parameters, canonical argument order, missing required arguments, wrong result
types, canonical codec round trips, and existing View resource consumers.

Validation at Jujutsu working change `tqkzspus`:

```bash
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-bundle --all-targets --no-fail-fast
cargo test -p arcweft-cli app::bundle::tests --lib -- --nocapture
cargo test -p arcweft-runtime-driver --test session \
  session_save_restores_complete_per_mount_virtual_range_state -- --exact
cargo check -p arcweft-lang-syntax -p arcweft-bundle -p arcweft-cli \
  -p arcweft-runtime-driver -p arcweft-player-web --all-targets
cargo clippy -p arcweft-lang-syntax -p arcweft-bundle -p arcweft-cli \
  -p arcweft-runtime-driver -p arcweft-player-web --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-definitions-2026-07-12
```

All commands pass. The syntax suite retains the explicit rejection of
`mod game::opening` and exercises canonical `mod game.opening`; the bundle
suite passes 80 unit tests and every integration suite, including 15 View codec
tests; and the focused CLI bundle suite passes 41 tests. The structural audit
records 1,250 Rust files / 627,602 physical Rust LOC, 0 errors, and 143 tracked
warnings. The audit README records exact changed-file sizes, current largest
workspace Rust files, their classifications/responsibilities, and the absence
of dependency-edge changes.

## Non-goals

There are no deferred items from the supplied implementation directive. Typst
`TypesetBlock` remains a separate document-rendering system and is not an
ordinary player text producer covered by this unification.
