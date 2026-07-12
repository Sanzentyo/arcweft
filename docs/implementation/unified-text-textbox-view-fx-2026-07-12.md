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
- [x] Cut 3: shaped shared text layout and glyphon engine
- [x] Cut 4: prepared text batch and all ordinary producers
- [ ] Cut 5: RichText/reveal/shared Fx and native registry removal
- [x] Cut 6: direct View text painter order and executable per-mount View
- [x] Cut 7: shared capture and prepared-layout Agent geometry
- [x] Cut 8: persistent TextBox View and hardcoded dialogue removal
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

All product display-text producers now converge through the shaped
project-font engine and one `PreparedTextBatch`. RichText, View, persistent
TextBox, choices, action buttons, editable inputs, Native/Web capture, and
Agent observation use the canonical prepared items. The overall goal remains
active for final legacy-layout/API cleanup and checked vertical-LR,
ruby/text-combine, and Fx visual evidence.

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

At that slice, Cut 3 remained open because `LaidOutText`, `layout_frame`, and
the old `LaidOutText -> GlyphArea` adapter still existed. Their direct removal
is recorded in the canonical-layout cleanup slice below; no compatibility
wrapper was added.

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

This earlier migration state is superseded by the canonical batch-only slice
below. The no-font stateless facade, ordinary `RenderTextBlock`, selectable
sidecar, styled-paragraph producer, and dual prepared-frame fields have now
been deleted, completing Cut 4 without a compatibility wrapper.

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

The renderer-local registry and native relayout half of Cut 5 is now closed by
Cut 7: `arcweft-render-native` and its effect/shader/motion/capture stores were
deleted directly. Cut 5 remains open only for removing the still-public
`RenderStyledParagraph`/legacy descriptor staging vocabulary from the shared
planner and renderer after all producer migrations are complete. Runtime-host
injection of additional typed render-resource programs also remains to be
connected to the shared table before external provider loading is complete.

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

### Shared prepared-frame capture and Agent geometry

Cut 7 is complete at Jujutsu change `xvloqypt` (parent revision `f35ccc09`):

- `arcweft-render-wgpu::SharedOffscreenCapture` accepts only a
  `PreparedFrame` plus a typed `CaptureRequest`. One shared-renderer submission
  produces exact Color pixels, and ordered prepared geometry derives ObjectId
  and Mask attachments with validated IDs, finite bounds, crop policy, and
  unambiguous RGBA identities;
- `arcweft-player-native` and the CLI no longer depend on
  `arcweft-render-native`. The entire unpublished crate and its independent
  `LineDisplayFrame` relayout, visual state, Fx registries, and raster path were
  deleted rather than retained behind a compatibility feature;
- native developer capture, CLI observe, MCP capture, and resource reads all
  plan the ordinary player frame, render it with `SharedRenderer`, and retain
  the same Color/ObjectId/Mask bytes. A later read never starts a different
  text renderer and a non-current historical page request is rejected;
- `PreparedTextOwner` records the frame-local text ID, stable semantic owner,
  parent, source origin, owner kind, and object bounds. Dialogue, View, and
  control producers register this evidence when they append the canonical
  prepared item;
- Agent TextBox/page/line/run/ruby/glyph/logical-cluster geometry now comes
  from `TextLayout` and its source map. View text exposes the same line/run/
  ruby/glyph/cluster families, including vertical orientation and vertical
  form. No line-count estimate, screenshot scan, or native-only layout API is
  used;
- capture-region order is derived from the actual prepared owner inventory:
  image, View painter order, semantic control, then dialogue, with shaped glyph
  order before ruby annotation order. Hidden reveal paint is excluded from the
  attachment rather than represented as visible geometry;
- the provisional selected-capture identities (`native_rich_text_observer`,
  `shared_web_gpu_scene`, and `native_wgpu_adapter`) were replaced directly by
  the one `shared_wgpu_prepared_frame` contract; and
- the Agent implementation was decomposed into retained-attachment,
  player-capture, dialogue-geometry, View-geometry, and external unit-test
  modules. The main changed production modules are below the 1,200-LOC warning
  threshold. The existing `arcweft-render-wgpu/src/geometry.rs` remains a
  reviewed 2,266-LOC warning-level owner below the 2,500-LOC error threshold;
  it already delegates dialogue, prepared text, controls, and action buttons
  to responsibility modules.

Validation for this cut:

```bash
cargo test -p arcweft-layout -p arcweft-agent-protocol \
  -p arcweft-render-wgpu -p arcweft-player-native \
  --all-features --lib --tests
cargo test -p arcweft-cli --all-features --lib
cargo test -p arcweft-cli --features native-capture --test check \
  agent_observe_native::agent_observe_shared_renderer_writes_dialogue_layer_masked_framebuffer_crop \
  -- --exact --nocapture
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-shared-capture-2026-07-12
git diff --check
```

All listed gates pass. The focused crate matrix passes 259 normal tests with 14
adapter-pinned tests ignored by policy, CLI passes 181 unit tests, the real-GPU
shared TextBox-layer smoke passes, and `just test-fast` passes 446 tests. The
structural audit scans 1,255 Rust files / 621,851 physical Rust LOC with 0
errors and 133 pre-existing warning-level findings. Exact changed-file metrics
and dependency edges are recorded in the linked audit directory.

Two older broad integration assertions remain intentionally red until Cut 8:
the full-grammar history assertion expects every previously emitted TextBox
entry in one observation, and ruby child capture addresses an observation-step
derived object ID. The final design forbids recreating those results through a
hidden renderer. Cut 8 must satisfy them through the persistent per-target
TextBox presentation store and stable entry identity. No renderer fallback or
compatibility alias was introduced for this intermediate cut.

### Persistent TextBox store and stable identity slice

The state-owning half of Cut 8 is complete at the working change after
`xvloqypt`; Rust-backed View composition and hardcoded dialogue-renderer removal
remain open:

- `BundlePresentationSnapshot.dialogue` was replaced directly by the
  runtime-driver-owned `TextBoxPresentationStore`. Each typed target retains an
  ordered entry list, active entry, monotonic revision, stable
  `TextBoxRuntimeId` / `TextBoxEntryId`, and persistent Rust-backed View mount;
- ordinary `DialogueLine` output applies ordered `Append` operations. Typed
  `Replace` and `Clear` operations are transactional and covered independently;
  no `frames.last()` projection remains;
- TextBox mounts are issued by the exact same allocator as authored View
  mounts. Save/restore validates the combined mount namespace, entry and
  occurrence cursors, active entries, display stages, Fx state, and allocator
  cursor before mutating the live session;
- advance targets capture textbox, entry, occurrence, stage, and TextBox
  revision. A repeated target after a stage or line mutation is rejected as
  `StaleRevision`, while unknown target/entry and non-waiting cases retain
  distinct typed rejection reasons;
- native, Web, player-scene, and CLI consumers now select typed active entries
  from the store. Reveal-first input remains player-owned: reveal completion
  changes the shared `DialogueVisualClock`; only a subsequent primary action
  queues the captured runtime target; and
- Agent object identity is now
  `object.dialogue.<TextBoxRuntimeId>.<TextBoxEntryId>` and is independent of
  the observation frame index. Capture URIs still use the real frame index, so
  an object observed at frame 3 remains `object.dialogue.0.0` while its retained
  resource URI remains under `frame/3`.

Validation for this slice:

```bash
cargo test -p arcweft-runtime-driver --all-features
cargo test -p arcweft-player-scene --all-targets --all-features --no-fail-fast
cargo test -p arcweft-player-web --all-targets --all-features --no-fail-fast
cargo test -p arcweft-player-native --all-targets --all-features --no-fail-fast
cargo test -p arcweft-cli --all-features --lib
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-persistent-textbox-store-2026-07-12
```

The runtime-driver passes 102 tests across its unit and integration targets,
including the focused store/View allocator coverage; player-scene passes 85
tests, Web 36, native 54, and CLI 181 unit tests. Workspace check and clippy
pass. The audit records 1,256 Rust files / 622,900 physical Rust LOC, 0 errors,
and 133 warning-level findings.

The first combined multi-package test command exceeded its 120-second command
limit while compiling and produced no test result; every package was then run
separately to completion as listed above. Two visual-suite facts remain open
and are not reclassified as success: the old ignored resource-matrix fixture
still expects the pre-convergence `framebuffer_crop`/legacy bounds contract,
and the active vertical-LR ruby/text-combine assertion expects one `2026`
cluster while current shared layout exposes four digit clusters. Cut 9 must
classify and resolve that layout difference with typed layout evidence and
reviewed raster output before any golden is changed.

### Standard TextBox View composition slice

The renderer-facing half of Cut 8 was connected at Jujutsu change
`uormqyrm`; the immediately following slice below deletes the legacy public
dialogue/styled-paragraph vocabulary and closes Cut 8:

- every active persistent target is prepared as a Rust-backed `ViewScene` using
  its retained `TextBoxViewMountId`. The panel is a normal View surface and the
  speaker/body are `ViewPrimitive::Text` references into the one canonical
  `PreparedTextBatch`;
- one target preserves the former 1280x720 panel geometry, insets, palette, and
  speaker/body metrics. Multiple targets use stable runtime-ID order and a
  bounded vertical tiling rule instead of overwriting each other;
- the body resolves the exact active `LineDisplayStage` and retains vertical
  flow, ruby, reveal paint, shared Fx evaluation, diagnostics, and logical
  source origin. The speaker is a separate canonical prepared item and keeps
  horizontal label flow, matching the former renderer behavior;
- `PreparedTextOwnerKind` now carries exact TextBox/entry/mount/part identity.
  Agent observation enumerates every active target and resolves its body owner
  by typed identity rather than a latest-entry or frame-step convention;
- auto-positioned choices no longer inspect `RenderScene.dialogue`. A generic
  `content_avoidance_regions` contract receives the same standard TextBox
  bounds, so choices retain their non-overlap behavior without making the
  geometry planner aware of a dialogue renderer; and
- a product-path integration test builds a persistent vertical-rl TextBox with
  ruby, prepares the ordinary player frame, and proves that the frame contains
  one panel plus speaker/body Text primitives, two typed owners, no legacy
  dialogue block, and the canonical vertical/ruby layout.

Validation for this slice includes:

```bash
cargo test -p arcweft-player-scene --all-features
cargo test -p arcweft-player-scene --test textbox_view --all-features
cargo check -p arcweft-render-wgpu --all-targets --all-features
cargo check -p arcweft-player-web --all-targets --all-features
cargo check -p arcweft-cli --all-targets --all-features
cargo check --workspace --all-targets --all-features
```

All listed commands pass. The complete player-scene route passes 88 tests,
including the new product TextBox View case. At that cut point,
`RenderScene.dialogue`, `RenderDialogue`, `RenderStyledParagraph`, their
renderer/report staging, and the temporary dialogue boolean facade remained;
the next slice removed them directly.

The structural audit at
`structure-audits/unified-text-textbox-view-composition-2026-07-12` scans 1,258
Rust files / 623,665 physical Rust LOC with 0 errors and 133 warnings. The new
TextBox owner is 531 LOC (499 production code LOC including a small embedded
test module), while the frame orchestrator remains 496 LOC. No Cargo manifest
or workspace dependency edge changed.

### Legacy dialogue staging removal and canonical Web evidence slice

Cut 8 is complete at Jujutsu change `xnrroynx` over revision `c3cbba0a`:

- `RenderScene.dialogue`, `RenderDialogue`, `RenderStyledParagraph`, styled
  spans/glyph transforms, `PreparedFrame::styled_paragraphs`, and the entire
  styled renderer/test path are deleted. There is no alias, dual reader, or
  no-op compatibility branch;
- `PreparedFrame` exposes direct persistent TextBox state queries for reveal
  and advance behavior. Input, keyboard, native capture, Web, and Agent use
  those typed entries rather than cached dialogue booleans;
- hardcoded dialogue panel/layer IDs, palette fields, geometry finalization,
  and choice inspection are gone. TextBox panels and speaker/body content are
  painted only by their normal View scenes, while generic content-avoidance
  regions retain choice placement behavior;
- Web parity checkpoints now drive the normal stateful `PlayerFramePlanner`
  and real `InputController`. Web frame observations project the completed
  `PreparedTextItem` layout and paint directly, including typed ownership,
  source ranges, lines, runs, glyph/ruby geometry, font inventory, visibility,
  orientation, vertical form, and the applied affine/opacity values;
- the standalone raster verifier consumes that canonical evidence and rejects
  count/range/style/font inconsistencies. It no longer recreates a styled
  paragraph or describes transforms as metadata-only;
- each prepared glyph submission retains a distinct glyphon renderer/vertex
  buffer until the shared command buffer is submitted. A local-adapter
  regression grows the second submission beyond the initial buffer and proves
  that earlier passes remain valid;
- Web redraws now wait for async GPU/font registration. Runtime redraw failures
  reach the shell's fatal DOM/diagnostic state, and the compositor WGSL uses
  browser-portable explicit precedence for multiply/XOR expressions; and
- the CSS parity capture registers the same ordered font set and uses the same
  1280x720 `Contain` frame fit on native and Web. Its 13 KiB deterministic Noto
  subset carries the fixture's `星影ほしかげ` coverage without replacing the
  full product font.

Validation for this slice includes:

```bash
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-player-scene --all-features
cargo test -p arcweft-render-wgpu --all-features
cargo test -p arcweft-player-web --all-features
cargo test -p arcweft-player-native --all-features
cargo test -p arcweft-render-wgpu --test prepared_text \
  multiple_prepared_submissions_keep_vertex_buffers_alive_until_submit \
  --all-features -- --ignored --exact --nocapture
cargo +nightly -Zscript tools/verify-text-raster-parity.rs --self-test
just css-style-parity
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-dialogue-staging-removal-2026-07-12
```

All commands pass. The CSS gate captures native and browser WebGPU at default,
compact, and HiDPI checkpoints. Every checkpoint reports 137 canonical glyph
runs with zero layout/raster delta; full-frame comparison reports PSNR
infinity, SSIM 1.0, MSE/MAE/maxAE 0, and zero changed pixels. `imq` JSON reports
are written for all three comparisons. The checked example evidence now uses
`prepared_text_glyph` sources and includes ruby plus an applied glyph
transform.

The first `just test-workspace` wrapper attempt exceeded its 600-second shell
limit during a cold all-test build; the child Cargo process completed after the
wrapper was terminated, so that wrapper invocation has no usable result. The
exact recipe components were then run separately: the non-CLI workspace
lib/integration command passed, followed by CLI lib/bin and all eight focused
CLI integration commands. This is recorded as a recovered timeout, not as a
passing wrapper invocation.

The linked structural audit scans 1,256 Rust files / 621,678 physical Rust LOC
with 0 errors and 133 warnings. Deleting the duplicated contracts reduces
`geometry.rs` to 2,196 LOC and the renderer to 1,742 LOC. The Web adapter adds
only downward dependency edges to `arcweft-glyphon` and
`arcweft-text-layout`; its exact fan-out is 28 and fan-in is 0. The following
slice removes the remaining ordinary staging path. Cut 9 still must
resolve/promote the vertical-LR ruby/text-combine and Fx visual goldens before
the overall goal can close.

### Canonical prepared-batch-only control text slice

Cut 4 is complete in the current working change:

- `PreparedFrame` owns exactly one public `text: PreparedTextBatch`; the dual
  `prepared_text` field and ordinary `RenderTextBlock`, selectable-text
  sidecar, font-family/weight/slant replicas, stateless planner facade, and
  renderer-local ordinary font system/cache/renderer are deleted directly;
- `SharedFramePlanContext` retains one project-font `GlyphonTextEngine` and
  private pre-shaping source plans. `prepare_mapped` maps those plans before
  shaping, so final fit, font size, clip, and device scale determine the one
  canonical layout and raster keys;
- choices, action buttons, and text inputs enter the prepared batch during
  frame planning in painter order and receive typed `Control` ownership;
- editable text resolves its displayed value once to a
  `ResolvedTextDocument`, lays it out through the shared engine, and derives
  editor hit/selection/caret/IME geometry from that same `TextLayout`.
  Single-line inputs use explicit no-wrap plus horizontal scrolling,
  multiline inputs wrap, and secure inputs prepare only their masked display;
- the shared prepared renderer paints selection before glyph submission and
  caret/composition marks afterward for both direct View text and ordinary
  batch ranges. Filtered controls replay the same prepared item and never
  reshape text;
- Native, Web, CLI Agent observation, fixtures, and the standalone parity
  verifier consume the same `frame.text` contract. Web observations expose
  one `text_count` and one canonical `text` collection; and
- source inventory confirms there is no production occurrence of the deleted
  staging contracts. Remaining `prepared_text` spellings identify the Agent
  observation object type, typed owner accessor, or Takumi's domain-local
  prepared collection rather than a second renderer input.

Validation for this slice includes:

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-render-wgpu --lib --tests
cargo test -p arcweft-player-scene --lib --tests
cargo test -p arcweft-player-web --lib --tests
cargo test -p arcweft-player-native --lib --tests
cargo test -p arcweft-cli --lib --bins
cargo test -p arcweft-render-wgpu --test prepared_text --all-features \
  prepared_batch_interaction_paints_selection_before_glyphs_and_ime_after \
  -- --ignored --exact --nocapture
cargo test -p arcweft-render-wgpu --test prepared_text --all-features \
  multiple_prepared_submissions_keep_vertex_buffers_alive_until_submit \
  -- --ignored --exact --nocapture
cargo test -p arcweft-render-wgpu \
  --test runtime_control_backdrop_gpu_smoke --all-features \
  prepared_control_foreground_filter_blur_executes_shared_renderer_path \
  -- --ignored --exact --nocapture
cargo +nightly -Zscript tools/verify-text-raster-parity.rs -- --self-test
just css-style-parity
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-canonical-batch-only-2026-07-12
```

All completed commands pass. The CSS gate captures Native and browser WebGPU
at default, compact, and HiDPI checkpoints; each full image is byte-identical
(PSNR infinity, SSIM 1.0, MSE/MAE 0, and zero changed pixels), and all 137
canonical text runs per checkpoint have zero raster-mask or geometry delta.
The local-adapter readbacks pass for batch interaction order, multiple glyph
submissions, and filtered control rendering. The linked structural audit scans
1,254 Rust files / 620,421 physical Rust LOC with 0 errors and 131 warnings.
It records every changed Rust file, current workspace hotspots, embedded test
LOC, responsibilities, and exact fan-in/fan-out. `geometry.rs` is 2,108 LOC,
the renderer is 1,516 LOC, and the rewritten text-control module is 756 LOC;
no Cargo manifest or dependency edge changed.

### Shaped vertical, ruby, and capture regression closure slice

The first Cut 9 implementation slice is complete in Jujutsu change
`nyoynlov`:

- canonical vertical layout now shapes all runs first and plans contiguous
  runs with the same writing mode and resolved JLREQ strictness as one
  paragraph. Paint, typewriter, and Fx run boundaries therefore cannot alter
  UAX/JLREQ column composition;
- paragraph break opportunities are derived from the complete resolved text,
  not recomputed from each styled substring. `2026XYZ` consequently remains
  one unbroken alphanumeric sequence even when `2026` carries a typewriter
  effect;
- the shaped-column DP uses real inline advances, text-combine cells,
  sideways-Latin extents, compression/hanging rules, prohibited line heads and
  ends, and authored loose/normal/strict JLREQ pair policy. `vertical_rl` and
  `vertical_lr` share that inline plan and differ only in column direction;
- side-track ruby reserves its physical track before body layout. Auto/over is
  right for `vertical_rl` and left for `vertical_lr`; under reverses those
  sides. Inter-character ruby instead reserves inline extent, is placed after
  the first base cluster, and pushes the following base cluster without a side
  track;
- nested layout cascade is now field-wise in both presentation and
  `ResolvedTextStyle`. An inner `.ruby_under` or `.ruby_inter_character` no
  longer resets an outer `.vertical_rl`/`.vertical_lr` to the default
  horizontal flow;
- Agent observation aggregates shaped glyphs by logical `cluster_index`, so
  text-combine `2026` and sideways `ABC`/`XYZ` have one stable cluster object,
  union source range, union bounds, and one hit region. View and dialogue use
  the same aggregation rule;
- capture visibility and painter ordering use the same cluster-index plus
  contained-source-range rule. A multi-glyph text-combine cluster can therefore
  be hidden at logical time zero and produce nonempty color/mask/object-id
  crops when visible without relayout; and
- shared mask/object-id attachments retain their documented semantic-region
  contract: the selected logical bbox is fully covered. The masked color crop
  retains actual rendered pixels. Tests no longer ask the shared capture path
  to recreate the deleted native-only glyph-alpha attachment.

The initial visual review used candidate-only files under
`target/unified-text-final-review`; no checked-in MS Mincho migration witness
was overwritten. The fixed vertical-LR candidate visibly separates `ゆめ` to
the physical left of `夢`, retains upright `2026`, and retains sideways Latin.
At fixed logical times, the existing effect candidate changes between 4.0s and
4.5s with PSNR 36.9419 dB, SSIM 0.97615, MSE 0.00020221, and MAE 0.00051983.
These are diagnosis artifacts, not the final promoted project-font goldens.

Validation for this slice includes:

```bash
cargo test -p arcweft-render-text --all-targets
cargo test -p arcweft-text-layout --all-targets
cargo test -p arcweft-glyphon --all-targets
cargo test -p arcweft-render-wgpu --all-targets
cargo test -p arcweft-player-scene --all-targets
cargo test -p arcweft-cli --features native-capture --test check \
  agent_observe_native::agent_observe_native_renderer_reports_vertical_goal_clear_smoke_geometry \
  -- --exact --quiet
cargo test -p arcweft-cli --features native-capture --test check \
  agent_observe_native::agent_observe_native_renderer_writes_vertical_goal_clear_smoke_raw_crops \
  -- --exact --quiet
cargo check --workspace --all-targets --all-features
cargo clippy -p arcweft-render-text -p arcweft-text-layout \
  -p arcweft-glyphon -p arcweft-render-wgpu -p arcweft-player-scene \
  -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-shaped-vertical-regression-2026-07-12
```

All completed commands pass. Additional focused native integration tests pass
for RL/LR column direction, vertical ruby collision, ruby-under physical
sides, inter-character flow, TextBox/ruby/vertical capture bounds, and the
vertical goal-clear mask/object-id crops. Several first CLI invocations reached
the 60-second shell limit immediately after a cold test-binary build; cached
reruns then executed and passed, so only the rerun results are counted.

The linked structural audit scans 1,255 Rust files / 621,347 physical Rust LOC
with 0 errors and 131 warnings. No manifest or dependency edge changed. The
largest changed production files are the 1,190-line canonical resolved-document
module and 1,185-line prepared-text observation module, both below the 1,200
LOC warning threshold. The new vertical planner is 254 LOC including a 64-line
embedded unit-test module.

This does not complete Cut 9. The remaining acceptance work is to promote a
checked project-font Native/Web/headless visual packet for vertical-RL,
vertical-LR+ruby+text-combine, a JLREQ pair that visibly distinguishes authored
strictness, and fixed-time Fx/reveal frames; then remove the obsolete
`layout_frame`/`LaidOutText` adapter instead of keeping a compatibility route.
The pre-existing, currently unconsumed `--textbox-height` Agent option is also
part of that cleanup decision: it must be connected to one typed TextBox layout
input or removed with its stale callers, not silently treated as evidence.

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

### Project-font unified visual-parity slice

The final-path visual witness is now independent of the historical native
system-font goldens:

- `samples/unified-text-visual-parity/main.arcw` contains five sequential
  pages covering vertical-RL UAX punctuation, vertical-LR ruby-under and
  inter-character ruby, four-digit text-combine-upright, sideways Latin,
  loose/strict JLREQ composition of identical source text, a source-defined
  glyph sampler, and delayed typewriter reveal;
- the fixture registers the deterministic checked-in
  `noto-sans-jp-unified-text-parity.ttf` project-font subset on both backends;
- the generalized native/headless and browser harnesses use
  `capture-text-parity-frame.rs` and `text-parity-smoke.mjs`. They stop at the
  target page activation tick, then advance the same 16 ms runtime clock
  quanta. The frame report records activation tick, capture tick, elapsed
  steps, and quantized elapsed milliseconds;
- source-defined RichText Fx applications and the bundle `FxDefinitions`
  inventory now receive one selected package identity during lowering. Direct
  sources use their file stem, projects use the manifest package, and project
  compilation enforces its manifest identity. The unpublished contract was
  corrected directly without runtime rebasing, an alias, or a dual reader;
- sideways vertical raster origins are recovered from the final ink rectangle
  while retaining the shaped face's left/top bearings. This aligns reported
  ink, capture pixels, and hit geometry after quarter-turn placement;
- `verify-unified-text-visual-parity.rs` checks semantic geometry and generated
  attachments in addition to the generic text-raster, full-frame, and IMQ
  gates. Representative TextBox-body scopes retain color, mask, object-ID,
  semantic owner, crop, and layout-hash evidence.

`just unified-text-visual-parity` passes all eight checkpoints:

| Evidence | Native/Web result | Native semantic/temporal MSE | Web semantic/temporal MSE |
| --- | --- | ---: | ---: |
| vertical-RL | pixel exact; 28 raster runs | — | — |
| vertical-LR | pixel exact; 27 raster runs | — | — |
| JLREQ loose vs strict | each checkpoint pixel exact | 0.0012708181 | 0.0012708181 |
| Fx 4000 ms vs 4500 ms request | each checkpoint pixel exact | 0.0003545461 | 0.0003545461 |
| reveal 20000 ms vs 20500 ms | each checkpoint pixel exact | 0.0005311874 | 0.0005311874 |

The 4500 ms Fx request is deterministically rounded up to 4512 ms by the
16 ms runtime quantum on both backends. The Fx layout hash remains stable while
the three `波動光` glyph transforms change; the reveal layout hash remains
stable while exactly one additional logical glyph becomes visible. Loose JLREQ
places `。` and `「` on different columns, while strict JLREQ keeps them on the
same column. Native and Web prepared glyph/ruby/run geometry, font
fingerprints, logical-clock records, and final pixels are identical at every
checkpoint.

Validation at Jujutsu working change `wyslyrwo`:

```bash
cargo test -p arcweft-text-layout --test document_layout --all-features
cargo test -p arcweft-runtime-plan \
  rich_text_fx_uses_the_selected_package_identity --lib
cargo test -p arcweft-cli \
  direct_bundle_source_defined_fx_application_resolves_its_definition \
  --all-features
cargo check -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-cli \
  --all-features
just unified-text-visual-parity
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-visual-parity-2026-07-12
```

All commands pass. The structural audit records 1,256 Rust files / 622,283
physical Rust LOC, 0 errors, and 131 tracked warnings; no Cargo dependency edge
changed. The visual packet itself is closed and reproducible without
overwriting any checked-in system-font golden.

### Canonical shaped-layout-only cleanup slice

Cut 3 is now complete. The repository no longer exposes or compiles the
font-independent estimated-layout route:

- `layout_frame`, `LaidOutText`, `LaidOutGlyph`, `LaidOutRun`, `LaidOutRuby`,
  and `TextLayoutConfig` are removed from `arcweft-text-layout` rather than
  retained as aliases;
- the old horizontal/vertical/ruby/effect-reserve planners and their dedicated
  test modules are deleted. Shared Unicode vertical orientation, grapheme
  clustering, generated JLREQ punctuation data, and the shaped
  `layout_document` planner remain the canonical implementation;
- `arcweft-glyphon` no longer exposes the unused `LaidOutText -> GlyphArea`,
  shaped-buffer, horizontal-buffer, or vertical-buffer adapters. Its 1,300-line
  root is now a 15-line facade over `prepared_text` and `text_engine`;
- the two crates no longer retain `arcweft-core` as a dev dependency used only
  by the deleted compatibility tests;
- the canonical test surface consists of shaped layout, real project-font
  cache/raster behavior, prepared text validation, vertical/JLREQ planning,
  ruby geometry, and the project-font visual packet. The fast route retires
  114 tests that exercised only the removed estimated algorithm.

Validation at Jujutsu working change `lwzommlm`:

```bash
cargo test -p arcweft-text-layout -p arcweft-glyphon \
  --all-targets --all-features
cargo check --workspace --all-targets --all-features
cargo clippy -p arcweft-text-layout -p arcweft-glyphon \
  -p arcweft-render-wgpu -p arcweft-player-scene \
  --all-targets --all-features -- -D warnings
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
just unified-text-visual-parity
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-legacy-layout-removal-2026-07-12
```

All commands pass. The focused shaped/prepared suites pass 38 tests,
`just test-fast` passes 308 tests, and all eight visual checkpoints retain
pixel-exact Native/Web frames plus the same non-zero JLREQ, Fx, and reveal
semantic differences. Cut 9 remains open for provisional legacy-Fx staging
vocabulary, followed by final workspace validation, structural audit, and
push. The structural audit records
1,244 Rust files / 613,600 physical Rust LOC, 0 errors, and 128 warnings; this
slice removes 12 Rust files, 8,683 physical Rust LOC, three warnings, and two
test-only dependency edges.

### Unconsumed Agent TextBox-height option removal slice

The Agent-only `textbox_height` / `--textbox-height` input is removed directly.
It was copied through four CLI commands and seven MCP schemas but never reached
the canonical TextBox View mount, resolved style, layout constraints, or
prepared batch. Keeping it would therefore advertise a silent no-op as a
layout control. The real typed inputs remain the source/project TextBox/View
configuration and the requested viewport; no alias, deprecated spelling, dual
schema, or fallback value was added.

The MCP schema behavior test now proves that no tool advertises the removed
field. Native Agent JLREQ helpers no longer accept a dummy height argument,
while their explicit viewport dimensions remain unchanged.

Validation at Jujutsu working change `xxwwmruw`:

```bash
cargo check -p arcweft-cli -p arcweft-agent-mcp \
  --all-targets --all-features
cargo test -p arcweft-agent-mcp --all-targets
cargo test -p arcweft-cli --lib --all-features
cargo test -p arcweft-cli --test check --all-features --no-run
cargo clippy -p arcweft-agent-mcp -p arcweft-cli \
  --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-textbox-height-removal-2026-07-12
```

All listed focused commands pass: the MCP suites pass 28 tests, the CLI library
passes 182 tests, and the complete `check.rs` integration binary compiles. The
structural audit records 1,244 Rust files / 613,514 physical Rust LOC, 0 errors,
and 128 warnings; no dependency edge changed.

`just test-cli-native` additionally exposed two visual-smoke assertions left
behind by the prepared-text migration: selected color composition is now the
documented `masked_framebuffer_crop`, and a layer object-ID crop currently
retains descendant rich-text colors while its metadata names only the direct
layer object. These failures are unrelated to the removed option, but the
object-ID mismatch is a real capture-contract defect and is the immediately
following Cut 9 repair rather than a waived failure.

### Layer capture identity regression closure

The follow-on repair distinguishes direct scope roots from descendant pixel
coverage. A `dialogue` layer selects the TextBox as its published object; its
glyph/cluster/ruby descendants still contribute coverage because they overwrite
the retained object-ID attachment in painter order, but their colors are
canonicalized to the owning selected root in the scoped layer attachment.
`selected_capture.mask.object_ids` is carried from the same root selection, so
pixels and metadata cannot drift through separate reconstruction logic.

Object scope continues to publish exactly the requested object identity even
when overlapping rich-text elements are included for complete coverage. View
scope retains each directly referenced object identity. Parent traversal is
bounded by the observed object inventory, so malformed cyclic ancestry cannot
stall capture.

The old smoke expectation that a selected layer color image was an unmasked
framebuffer crop was corrected to the already documented and implemented
`masked_framebuffer_crop`; this is independently verified by nonempty pixels,
crop geometry, attachment flags, and selected-source metadata.

Validation at Jujutsu working change `vkwpxwkn`:

```bash
cargo test -p arcweft-cli --lib --all-features \
  shared_layer_object_id_uses_direct_object_identity_for_descendant_coverage
cargo test -p arcweft-cli --test check --all-features \
  agent_observe_native::visual_smoke -- --nocapture
just test-cli-native
cargo test -p arcweft-cli --lib --all-features
cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-layer-capture-identity-2026-07-12
```

All commands pass. The CLI library passes 183 tests, both real renderer visual
smokes pass, and the native CLI recipe passes its additional selected rich-text
capture case. The structural audit records 1,244 Rust files / 613,593 physical
Rust LOC, 0 errors, and 128 warnings; no dependency edge changed.

### Typed RichText built-in Fx compilation slice

The RichText wave, shake, jitter, arc, spin, pulse, motion, typewriter,
sparkle, shader, and post-process surface now compiles into ordinary typed
`FxDefinition` graphs. Reachable dialogue tags are collected from nested HIR
flow bodies into the same bundle inventory as authored `#[fx]` functions. Each
line retains a zero-parameter `FxApplication` whose definition ID is derived
from the complete canonical semantic key; the inventory and line lowering
must reproduce an identical definition or lowering fails.

Built-ins use the shared value-program evaluator for time, logical glyph
ordinal, checked integer noise buckets, typed transforms, masks, and colors.
The new `FloorToI32` and `MakeColor` instructions reject overflow and invalid
opacity channels through structured diagnostics; they do not clamp or return
zero. Shader resources and post-process operations resolve through the shared
typed renderer-resource table. An unknown shorthand keeps its exact missing
definition identity, while `phase=host_event` remains a typed host event; a
visual `.host id=sparkle` spelling no longer falls back to a built-in basename.

The removed `run` effect target and `state`/`scope`/`state_scope` metadata now
fail with structured lowering errors. Project samples use `content` and stable
per-occurrence `FxInstanceId` ownership. Inferred text-proxy structs retain
priority over generic inferred-effect classification. The sema span tracker
also distinguishes authored empty-attribute tags such as `[strong]` and `[em]`
from inferred dot-tag marks, so nested formatting inside an Fx span is checked
against its real authored stack. The built-in compiler is split into a
652-line graph composer, a 197-line typed attribute parser, and a 190-line
sampler-expression builder.

Validation at Jujutsu working change `pzllnvpo`:

```bash
cargo test -p arcweft-presentation -p arcweft-render-text \
  -p arcweft-runtime-plan --all-targets --no-fail-fast
cargo clippy -p arcweft-presentation -p arcweft-render-text \
  -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
cargo test -p arcweft-lang-sema --all-targets
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
target/debug/arcw.exe check samples/rich-text-full-grammar.arcw
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-builtin-fx-compiler-2026-07-12
```

All listed commands pass. The presentation/render-text/runtime-plan suites pass
361 tests and the sema suites pass 510 tests. The full RichText grammar sample
passes direct CLI check. The effects-animation sample remains blocked by its
already documented unresolved `shader.source_glow` entity, and the
modern-feedback project now passes the corrected span check before reaching
its independent pre-existing untyped `player_viewport` effect-boundary error.
The structural audit records 1,249 Rust files / 615,551 physical Rust LOC, 0
errors, and 128 warnings; the four affected crates retain fan-out/fan-in counts
of 7/8, 6/17, 5/16, and 9/7 respectively, with no Cargo edge change. Cut 5 remains open for
direct deletion of the provisional public effect/shader descriptors and the
renderer-side `dialogue_legacy_fx` interpreter now superseded by this typed
path.

## Non-goals

There are no deferred items from the supplied implementation directive. Typst
`TypesetBlock` remains a separate document-rendering system and is not an
ordinary player text producer covered by this unification.
