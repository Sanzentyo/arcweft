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
- [ ] Cut 6: direct View text painter order and executable per-mount View
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

The shaped layout data contract and a preliminary `ResolvedTextDocument`
consumer now exist, but the real glyphon shaper, prepared text batch, View Fx
evaluator/application mounting, RichText shared-plan execution, renderer
migration, and TextBox convergence remain open. The overall goal therefore
remains active.

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

## Non-goals

There are no deferred items from the supplied implementation directive. Typst
`TypesetBlock` remains a separate document-rendering system and is not an
ordinary player text producer covered by this unification.
