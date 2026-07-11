# Unified Text / TextBox / View / Fx final design

Date: 2026-07-12

Design baseline: `d934189ba1e414bfa23f7792658e69fd8c60d714`

Status: final implementation contract

This document closes the remaining implementation decisions in the unified
Text / TextBox / View / Fx directive. It replaces provisional renderer-local
text and Fx contracts directly. Arcweft has not released the affected APIs or
wire formats, so no compatibility alias, dual reader, deprecated wrapper, or
migration version is part of the design.

## 1. Final invariant

Every ordinary player path uses this one data flow:

```text
LineDisplayStage / ViewTextSource / localized text / TextEditor
    -> ResolvedTextDocument
    -> layout-phase ResolvedFxPlan
    -> TextLayout through GlyphonTextEngine
    -> PreparedTextItem
    -> PreparedTextBatch
    -> ViewPrimitive::Text(PreparedTextId)
    -> ViewCompositor
    -> SharedRenderer
    -> native / Web / shared offscreen attachments
```

The same `PreparedTextItem` is the source of raster submission, interaction
geometry, accessibility geometry, Agent observation, color capture, mask
capture, and object-id capture. Native code owns only native device/window
hosting, provider loading, command submission, and readback.

## 2. Ownership and dependency direction

| Crate | Final responsibility |
| --- | --- |
| `arcweft-render-text` | Resolved text document, style cascade result, dialogue stage projection, source ranges |
| `arcweft-text-layout` | Generic Sans I/O shaping contract, line/ruby/vertical layout, source map, layout hash |
| `arcweft-glyphon` | Project font inventory, glyphon/cosmic shaping backend, stable font/glyph keys, prepared text batch |
| `arcweft-presentation::fx` | Typed Fx graph/value/program model, evaluator, plans, diagnostics, instance snapshots |
| `arcweft-view` | Executable View value programs, per-mount evaluator, mount-based Fx identity |
| `arcweft-bundle` | Deterministic `FxDefinitions` and executable View program sections/codecs |
| `arcweft-runtime-driver` | Logical clock, live Fx/TextBox/View instance stores, atomic save/restore |
| `arcweft-player-scene` | Source-store joins and frame preparation orchestration |
| `arcweft-render-wgpu` | View composition and resolved-plan GPU submission |
| `arcweft-render-native` | Native WGPU host and readback adapter only |

`arcweft-presentation` remains Sans I/O. The shared Fx evaluator stays there
rather than creating a second evaluator crate. `arcweft-save` remains a generic
data-format crate; the runtime driver owns projection of live presentation
state into typed save records.

## 3. Canonical text document

`arcweft-render-text` owns the only post-resolution source model:

```rust
pub struct ResolvedTextDocument<'a> {
    text: &'a str,
    source_origin: usize,
    runs: Vec<ResolvedTextRun>,
    ruby: Vec<ResolvedTextRuby>,
    revision: TextDocumentRevision,
}

pub struct ResolvedTextRun {
    pub range: RichTextRange,
    pub source_range: RichTextRange,
    pub style: ResolvedTextStyle,
    pub presentation: RichTextPresentation,
    pub source: ResolvedTextRunSource,
}

pub struct ResolvedTextRuby {
    pub base_range: RichTextRange,
    pub source_base_range: RichTextRange,
    pub text: String,
    pub style: ResolvedTextStyle,
    pub presentation: RichTextPresentation,
}
```

`range` is document-local. `source_range` remains relative to the owning source
record. A dialogue stage borrows a slice of `LineDisplayFrame::text`, rebases
clipped metadata, and does not clone the full frame text. `source_origin`
provides the exact inverse mapping.

`LineDisplayFrame::resolve_stage_document` and
`RichTextDocument::resolve_document` are inherent domain behavior. View,
localized, generated, and editable sources use a resolver context implemented
above `arcweft-render-text`; the lower crate does not depend on View, bundle,
or player crates.

`ResolvedTextStyle` is closed and contains the font family stack, size, line
height, weight, slant, color, spacing, writing mode, inline direction, and
language. Renderer-local font/style enums are not renderer input.

## 4. Font inventory, shaping, and layout identity

### 4.1 Project font inventory

Product-conformant native, Web, and headless paths use the same ordered project
font inventory. System font discovery is a developer fallback that emits an
observable conformance diagnostic and cannot produce a release golden.

`FontFaceId` is the BLAKE3 digest of canonical font bytes, face index, and
selected variation coordinates. It is not a glyphon/fontdb process-local ID.
`FontInventoryHash` hashes the ordered face identities and shaping features.
Prepared glyphs retain Arcweft-owned stable keys; submission maps them to the
renderer-local glyphon cache only through the same inventory.

### 4.2 Shaper boundary

```rust
pub trait TextShaper {
    type Error: std::error::Error + Send + Sync + 'static;

    fn shape_run(
        &mut self,
        request: TextShapeRequest<'_>,
    ) -> Result<ShapedTextRun, Self::Error>;
}
```

`GlyphonTextEngine` owns one `FontSystem`, one `SwashCache`, and bounded shape
and raster-key caches. Horizontal, bidi, fallback, ligature, combining-mark,
emoji, CJK, vertical-form, and ruby placement consume shaped cluster metrics.
The current character-class estimated horizontal advance is removed.

### 4.3 Layout output and hash

`TextLayout` contains lines, runs, logical clusters, raster glyph placements,
ruby placements, bounds, source map, and `TextLayoutHash`. Logical ordinal is
stored independently from visual order so bidi does not change Fx identity.

The layout hash covers:

- document revision and resolved style;
- font inventory hash and shaping features;
- constraints, writing mode, language, and direction;
- only `before_layout` and `layout_transform` Fx output;
- canonical source map and resulting geometry using finite `f32` bits.

Reveal position, selection, caret, composition, hover, pressed state, opacity,
color, and paint-only Fx time are excluded. Cache tests must prove those
inclusions and exclusions directly.

## 5. Prepared text and View painter order

`arcweft-glyphon` owns:

```rust
pub struct PreparedTextId(u32);

pub struct PreparedTextBatch {
    items: Vec<PreparedTextItem>,
}

pub struct PreparedTextItem {
    pub layout: TextLayout,
    glyphs: Vec<PreparedGlyph>,
    pub paint: TextPaintPlan,
    pub interaction: TextInteractionPlan,
    pub clip: Option<LayoutRect>,
}
```

`PreparedTextId` is frame-local. Stable cross-frame identity belongs to the
source/View/TextBox/Fx instance records, not this index.

`PreparedFrame` contains one `PreparedTextBatch`; it contains no parallel plain,
styled, selectable, or dialogue text list. Selection rectangles, caret,
composition underlines, and character bounds live in the corresponding text
item.

`ViewPrimitive::Text(PreparedTextId)` is rendered when the compositor reaches
that primitive. The callback receives the active transform, clip, opacity,
blend, mask, offscreen group, and object-id context. Text is not deferred to a
frame-wide pass. `GlyphRun` and its sidecar handoff, plus separate selection,
caret, and composition primitives, are removed.

## 6. Typed Fx numeric model

### 6.1 Finite values

All runtime scalar values use `f32`. The public value wrappers store canonical
IEEE-754 bits so they can implement exact equality and deterministic hashing:

```rust
pub struct FiniteF32(u32);
pub struct Length(FiniteF32); // logical px
pub struct Angle(FiniteF32);  // radians
pub struct Seconds(FiniteF32);
```

Construction and deserialization reject NaN and both infinities. Negative zero
canonicalizes to positive zero. Decimal parsing and unit conversion happen
once during compilation. Runtime evaluation never reparses source text.

Accepted sampler units are:

| Expected type | Accepted source units | Canonical representation |
| --- | --- | --- |
| `f32` | no unit | `FiniteF32` |
| `Length` | `px` | logical pixels |
| `Angle` | `rad`, `deg`, `turn` | radians |
| `Duration`/delay slot | `s`, `ms` | seconds |

Contextual units such as `%`, `em`, `rem`, `ch`, and viewport units are not
accepted in sampler programs. They must be resolved by a typed pre-layout
style context before becoming `Length`, otherwise compilation fails. There is
no implicit zero or raw-token fallback.

Overflow, non-finite results, underflow of a non-zero literal to zero, division
by zero, invalid opacity, and unit/type mismatch are structured errors. An
explicit authored `clamp` instruction is permitted; evaluators never apply an
implicit clamp.

### 6.2 Closed runtime value set

Executable programs use a closed value enum covering `Bool`, `I32`, `F32`,
`Length`, `Angle`, `Seconds`, `Color`, `Vec2`, and `Transform2D`. Resource IDs,
selectors, and strings may appear in static graph properties but are not
numeric stack values.

The old `Integer(String)`, `Decimal(String)`, string-valued scalar, and
source-label `Binding(String)` representations are replaced directly.

## 7. Value programs and evaluator budgets

One validated instruction model is shared by Fx sampler programs and reactive
View values. `FxSamplerProgram` and `ViewValueProgram` are distinct validated
owners over the common instruction representation; neither is a type alias or
source-text wrapper.

The instruction inventory is closed:

- constants and typed parameter/state/context slot loads;
- `neg`, `add`, `sub`, `mul`, `div`;
- `abs`, `min`, `max`, explicit `clamp`;
- `sin`, `cos`, `floor`, `fract`;
- equality/order comparisons and boolean `not`, `and`, `or`;
- typed `select`;
- deterministic hash-noise from seed, ordinal, and integer bucket;
- `Vec2` and `Transform2D` construction;
- one typed `return`.

There are no loops, recursion, indirect calls, allocation, wall clock, host
callbacks, source evaluation, or arbitrary bytecode escape.

Arithmetic permits same-type addition/subtraction, dimensionless scaling of a
unit value, division of a unit value by `F32`, and division of equal canonical
unit types to `F32`. Unit multiplication and mixed-unit addition fail
validation. `ctx.time` is an activation-relative `F32` measured in logical
seconds, and `ordinal_phase()` returns a dimensionless `F32` interpreted as
radians by trigonometric intrinsics; this preserves the documented sampler
surface without treating wall time as a unit-bearing value.

Default hard limits are:

| Resource | Limit |
| --- | ---: |
| Fx definitions per section | 4,096 |
| Parameters per definition | 64 |
| Expanded graph nodes per definition | 4,096 |
| Expanded graph depth | 64 |
| Total graph nodes per section | 65,536 |
| Instructions per sampler program | 1,024 |
| Constants per sampler program | 256 |
| Stack values per program | 64 |
| Captured/parameter slots per sampler | 64 |
| Instructions per View value program | 4,096 |
| State projections per View program | 256 |
| Evaluator operations per Fx instance per frame | 262,144 |

Decode validates limits, types, control flow, stack height, slot bounds, and
the declared return type before any program can execute. Runtime budget
exhaustion is a typed diagnostic, never partial output.

## 8. Fx time, ordinal, identity, and seed

`ctx.time` is elapsed logical seconds since that Fx instance was activated:

```text
max(0, runtime_logical_time - activation_logical_time)
```

The runtime logical clock is deterministic, pause-aware, saveable, and
restored atomically. It is never wall clock. With reduce motion enabled,
sampler time is exactly `0.0`; static graph nodes still apply.

Ordinal is target-local logical order, never a UTF-8 byte offset:

- glyph: shaped glyph logical order within the application content, before
  bidi visual reordering;
- line: logical line index within the application content;
- node/content/background/viewport: zero unless a repeated retained owner
  supplies an authored child ordinal.

The golden-angle constant is fixed by bits:

```rust
pub const FX_GOLDEN_ANGLE_RAD: f32 = f32::from_bits(0x4019_98ff); // 2.3999631
```

`ordinal_phase()` evaluates
`(ordinal as f32 * FX_GOLDEN_ANGLE_RAD).rem_euclid(TAU)`.

View Fx identity contains `ViewMountId`, node key, optional repeat key, authored
ordinal, and optional local key. RichText identity contains TextBox runtime ID,
entry ID, authored node index, authored Fx ordinal, and optional local key.
Stage/page index is not part of the default identity.

The default seed is the first 64 little-endian bits of BLAKE3 over domain
`arcweft.fx-seed.v1`, `FxInstanceId`, semantic hash, optional authored seed
bytes, and the nested graph child path. Nested child path is a bounded sequence
of authored child ordinals and is included in save state.

## 9. Transform2D and composition

The closed transform value is:

```rust
pub struct Transform2D {
    pub translate_x: Length,
    pub translate_y: Length,
    pub scale_x: FiniteF32,
    pub scale_y: FiniteF32,
    pub skew_x: Angle,
    pub skew_y: Angle,
    pub rotation: Angle,
    pub origin_x: Length,
    pub origin_y: Length,
    pub opacity: FiniteF32,
}
```

Defaults are identity: zero translation/skew/rotation/origin, unit scale and
opacity. `opacity` must be in the closed interval `[0, 1]`; no clamp is applied.
Skew remains because it is an existing advertised transform feature.

For a column-vector point, one transform is:

```text
T(origin) * T(translation) * R(rotation) * K(skew) * S(scale) * T(-origin)
```

This is origin move, scale, skew, rotation, translation, and origin restore.
If skew is identity, it is exactly the required origin → scale → rotation →
translation → restore order. Opacity multiplies separately.

Fx stack is applied in authored order: the next authored transform is applied
after the previous one, so `M_total = M_n * ... * M_2 * M_1`. Each child uses
its own origin before matrix composition.

## 10. Targets, capabilities, and interaction geometry

The final target enum is closed:

```text
.node        whole retained View/TextBox node
.content     the application content group
.background  the node background paint group
.line        one logical text line
.glyph       one shaped glyph
.viewport    the compositor viewport
```

`.content` is the shared meaning of a View content subtree and a RichText Fx
span/run. Provisional document/run/textbox/screen/sentence target vocabulary is
replaced, not aliased. Default target is `.content`.

Renderer interfaces are typed enum values, not strings: text/style, color,
transform, mask, filter, shader-uniform, offscreen-pass, post-process,
transition, and geometry-transform. ABI hashing uses their canonical numeric
discriminants and property schemas.

Capability rules:

| Target | Geometry | Paint operations | Interaction behavior |
| --- | --- | --- | --- |
| node | transform/opacity | mask, filter, offscreen, transition | same transform applies to hit, focus, accessibility, clip |
| content | transform/opacity | color, mask, filter, offscreen | descendant hit/focus/accessibility geometry transforms |
| background | visual transform/opacity | color, mask, filter, offscreen | no hit geometry change |
| line | post-layout visual transform | color, mask, line offscreen | layout, selection, caret, hit geometry unchanged |
| glyph | post-layout visual transform | color, mask | layout, selection, caret, hit geometry unchanged |
| viewport | none except compositor transform | post-process, transition | viewport/input coordinate contract transforms together |

An interactive node/content transform must be invertible. A zero or otherwise
non-invertible scale on interactive geometry produces an error diagnostic and
the Fx application is not committed. Unsupported target/interface pairs also
produce an FxId/FxInstanceId-bearing diagnostic. They never silently no-op or
fall back to an unrelated builtin.

## 11. Fx graph output and failure semantics

Evaluation order is fixed:

```text
1. source/style resolution
2. parameter binding and graph instantiation
3. before_layout / layout_transform
4. shaping, layout, and ruby
5. glyph_transform
6. glyph_color
7. glyph_mask / reveal
8. run offscreen pass
9. post-process
10. compositor transition
```

The evaluator returns one `ResolvedFxPlan` containing typed layout, glyph,
mask, offscreen, post-process, transition, and diagnostic records. Constructor
property schemas are closed:

- style/text: opacity, weight, slant, font family, size, spacing, color;
- color: tint/multiply color and opacity;
- transform: phase, target, static transform or sampler program;
- mask: phase, target, typed resource/coverage program, invert flag;
- filter: blur radius, brightness, contrast, saturation;
- shader: typed resource ID, stage, closed uniform map;
- transition: kind, duration, easing, progress program;
- conditional: boolean value program and two graphs;
- stack: authored ordered children.

Unknown constructor properties fail compilation. Unknown external resources
remain typed IDs and fail provider/capability resolution visibly.

Evaluation is transactional per application. Numeric, budget, ABI, missing
definition/provider, or capability failure produces no partial plan. Base
content may still render without that treatment, but the frame carries an
error diagnostic and exact-conformance capture fails. This is a diagnosed
failed application, not a silent identity fallback.

## 12. Bundle and symbol identity

Program bundles always contain one independent `FxDefinitions` section, even
when empty. It becomes a required Program section and stores:

- `FxId`, ABI hash, semantic hash;
- closed parameter/default schemas;
- complete typed graph and value programs;
- referenced resource IDs;
- required renderer interfaces and provider policy.

The initial supported section schema directly replaces the provisional graph
sidecars. No old reader, alias, or schema-version bump is introduced.

The View section becomes a multi-program executable inventory. Digest-only
expression references are replaced with `ViewValueProgram` records for params,
locals, props, conditions, keyed-repeat source/key, calls, await branches, and
Fx argument slots. Runtime re-evaluates only dirty slots; graph identity and
`FxInstanceId` do not change when a value changes.

HIR and linking retain an original `CallableDeclarationId` consisting of the
canonical package identity and original module-qualified declaration. Imports,
grouped imports, aliases, glob imports, and `pub use` bind aliases to that same
ID. They never mint a new `FxId`. Ambiguous unqualified imports are errors.
Direct-file developer builds use an explicit build-provided package identity;
project and dependency builds use their canonical package records.

## 13. Provider ABI

Rust, WASM, and builtin providers register the same descriptor:

```rust
pub struct FxProviderDescriptor {
    pub id: FxId,
    pub abi_hash: FxAbiHash,
    pub semantic_hash: Option<FxSemanticHash>,
    pub kind: FxProviderKind,
    pub interfaces: FxRendererInterfaceSet,
    pub limits: FxProviderLimits,
}
```

Providers receive typed values, sample context, target metadata, and bounded
output storage. They return only Arcweft-owned plan operations. No provider
receives a WGPU device, encoder, glyphon buffer, raw glyph callback, or RGBA
callback. Duplicate IDs, ABI mismatch, output budget overflow, unavailable
native-only providers, and unsupported interfaces are typed errors. Native and
Web adapters may load different provider implementations only when they expose
the same descriptor/ABI and produce the same typed plan contract.

## 14. Runtime state and save/load

Each live instance stores:

```rust
pub struct FxInstanceSnapshot {
    pub instance: FxInstanceId,
    pub definition: FxId,
    pub abi_hash: FxAbiHash,
    pub activation_logical_time: FxLogicalTime,
    pub deterministic_seed: u64,
    pub parameters: Vec<FxRuntimeValue>,
    pub child_path: FxGraphChildPath,
    pub provider_state: Vec<FxProviderStateRecord>,
}
```

Provider state is typed, bounded, provider-versioned data; it is not an opaque
native pointer. The runtime session save atomically includes TextBox entries,
View mounts/local state, Fx instances, reveal state, transitions, and logical
clock. Restore validates all definition IDs and ABI hashes before mutating the
live session. A single mismatch rejects the atomic restore with a typed
diagnostic.

All Fx diagnostics include a stable code, severity, definition ID when known,
instance ID when known, graph child path, target/interface when relevant,
source range when available, and message. The same records flow to native,
Web, headless, save errors, and Agent observation.

## 15. TextBox as a persistent View implementation

`TextBox` remains a dialogue domain/output/lifecycle object. Its display is a
persistent View mount. No `ViewElementKind::TextBox` is added.

```rust
pub struct TextBoxPresentationStore {
    textboxes: BTreeMap<TextBoxRuntimeId, TextBoxPresentation>,
}

pub struct TextBoxPresentation {
    pub revision: TextBoxRevision,
    pub entries: Vec<TextBoxEntryState>,
    pub active: Option<TextBoxEntryId>,
    pub mount: ViewMountId,
}
```

Runtime output is applied as ordered append, replace, and clear operations; no
`frames.last()` projection is allowed. Each target has an independent mount,
local state, focus state, and Fx state. Standard TextBox is a Rust-backed View
using the same props schema and `ViewPrimitive::Text` as authored Views.

Primary action completes reveal while reveal is active, then advances the
captured target occurrence/stage/revision. Stale actions are rejected.
Dialogue page/wait/speed semantics stay in the dialogue/runtime owner and do
not move into layout or renderer code.

## 16. Shared capture and Agent geometry

`NativeOffscreenCaptureSession` may retain native device/queue/readback
ownership, but its only frame input is `PreparedFrame`. Shared capture supports
Color, ObjectId, and Mask attachments plus frame/layer/View/TextBox/run/ruby/
glyph scopes. Alternate attachments are generated from the same prepared
geometry and painter order.

Agent observation derives text objects and bounds from `TextLayout` and its
source map. It never estimates height from line count, scans screenshot pixels
for geometry, or calls a native layout API. If a capture is requested after a
headless observation, the host retains or deterministically regenerates the
same prepared frame; it does not start a different text renderer.

## 17. Visual regression policy during migration

Before deleting the old renderer, create a candidate-only witness packet from
`d934189b` containing:

- the four checked-in vertical/JLREQ/ruby fixtures;
- `vertical_goal_clear_smoke` at logical times 0 and 4 seconds;
- `rich-text-effects-animation` at 0, 0.125, 0.375, 1, 4, and 4.5 seconds;
- color plus representative mask/object-id captures;
- observe JSON, source hashes, PNG hashes, `imq` JSON, viewport/DPR, font hash,
  GPU/backend, reduce-motion state, command, and revision.

Existing MS Mincho/native-only images are migration witnesses, not the final
cross-backend oracle. Final goldens use checked-in project font bytes and the
shared path. A visual difference is classified as semantic regression,
unintended raster regression, or intentional shaping correction. Only the last
may update a golden, with reviewed before/after images and metrics.

Final evidence has three layers:

1. exact typed layout/source-map/interaction/hash parity across native, Web,
   and headless;
2. same-frame color/object-id/mask attachment and crop evidence;
3. pinned project-font raster comparison with `imq` metrics.

Golden tests regenerate and compare the artifact itself. No test searches
implementation source for removed symbol spellings or file locations.

## 18. Implementation cuts and completion gate

Implementation proceeds in reviewable, compiling cuts:

1. migration witness packet and canonical resolved document;
2. typed Fx programs, evaluator, bundle section, symbol identity, persistence;
3. shaped shared layout and `GlyphonTextEngine`;
4. `PreparedTextBatch` and producer/interaction migration;
5. RichText/reveal/Fx migration and removal of renderer-local registries;
6. direct View text painter order and live per-mount View evaluator;
7. shared capture/Agent geometry and native adapter reduction;
8. persistent TextBox View composition and legacy cleanup;
9. cross-backend visual parity, stable docs, and structural audit.

Completion requires all directive conditions, all typed/behavior tests, all
relevant visual gates, and absence of legacy production paths by direct code
review. Absence is not enforced with a source gate.

## 19. Deliberate clarifications to the directive

- The shared Fx evaluator is in `arcweft-presentation::fx`, which is already a
  low-level Sans I/O owner; no duplicate evaluator crate is added.
- `skew_x` and `skew_y` remain in `Transform2D` to preserve the existing
  advertised skew feature. Their exact position in composition is specified.
- `.viewport` is retained as a typed compositor target for transitions and
  post-processing; the required `.node`, `.glyph`, `.line`, `.content`, and
  `.background` targets remain the ordinary View/Text target set.
- Context-dependent length units are rejected in sampler IR until resolved by
  a typed pre-layout context. They are never guessed or silently converted.
