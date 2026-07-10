# Dialogue control playback — 2026-07-10

## Status and scope

This slice makes the built-in dialogue playback controls observable behavior in
the shared interactive native/Web player path. It replaces the earlier model
where `[p]`, `[l]`, `[w]`, `[clear]`, and `[speed ...]` survived lowering but
did not all participate in player progression or reveal.

The slice owns display-stage progression and reveal-local controls. It does not
claim that every zero-width dialogue event is dispatched, nor does it redesign
generic View text policies, source/stream lowering, or the older
`arcweft-dialogue` facade model. Those boundaries are recorded below so they do
not become accidental completion claims.

## Fixed language contract

- `[p]` is a user wait that always closes the current logical page. If content
  follows, the next stage starts a new logical page.
- `[l]` is a user wait that retains the current logical page. Text visible
  before the marker remains part of the page when the next stage reveals.
- A terminal `[p]` is the final input gate for the line. It creates no empty
  page; advancing releases the line to the runtime continuation.
- `[w]` starts only when reveal reaches the marker, waits automatically, and
  then resumes. Its duration must be positive and use `ms` or `s`.
- `[clear]` resets displayed text immediately when reveal reaches the marker.
  It is not an input wait or a logical-page boundary. `[er]` and `[cm]` are
  aliases. Its display origin persists across a later `[l]` on the same page.
- `[speed ...]` changes the reveal rate for subsequent text until another
  speed/reset boundary or the end of the line. It accepts the named rates
  `slow`, `normal`, and `fast`, or a checked numeric rate from 1 through 240
  characters per second.

Logical pages are authored semantics. The removed `page_policy` field never had
an implemented authoritative runtime owner and is not replaced. A TextBox theme
may style or animate a page transition, but cannot change any rule above.

## Implemented ownership boundary

The implementation keeps the existing crate direction instead of making
`arcweft-core` depend on presentation types:

1. `arcweft-lang-syntax::ast::dialogue` parses `[w]` durations into a positive
   millisecond value, and `arcweft-lang-sema::checker::line_plan` reports
   malformed control attributes before runtime-plan lowering.
2. `arcweft-runtime-plan::render_text::tag` lowers controls into typed
   `RichTextControl` values. Display-map markers carry exact UTF-8 text offsets.
3. `arcweft-render-text::playback` derives input-gated `LineDisplayStage`
   projections from a resolved line. Page and line waits remain distinct, and
   terminal page waits do not produce a synthetic tail stage. It also validates
   restored display-map UTF-8 ranges, authored ordering, typed controls, ruby,
   host events, and derived stage ranges before playback.
4. `arcweft-runtime-driver::dialogue` owns dialogue occurrence, stage, and page
   identities. Advance input is targeted to the observed occurrence and stage,
   so stale input is rejected. Intermediate advances change presentation
   stage; only the terminal advance queues the canonical line-targeted core
   release. Raw `advance`/`dialogue.advance` events cannot bypass this boundary.
5. `arcweft-player-scene::dialogue` owns the shared native/Web stage-local
   reveal clock. Snapshot data stores elapsed stage time rather than a host
   absolute timestamp.
6. `arcweft-render-wgpu::geometry::dialogue_timeline` evaluates reveal speed,
   reached timed waits, and reached clear markers for the current stage.
7. The runtime-driver presentation snapshot preserves occurrence/stage/page
   progression and is validated against the active runtime dialogue when a
   session save is created or restored. Agent observation consumes the current
   stage projection and its advance actionability rather than reconstructing
   progression from source text.

This is a presentation progression boundary around the existing line-level
core suspension. The core receives an advance only when the current display
stage is terminal; it does not parse or search rich-text source.

## Verification boundary

Behavioral tests for this slice must cover:

- distinct `[l]` and `[p]` stage/page transitions;
- no empty tail after a terminal `[p]`;
- stale occurrence/stage advance rejection and exactly one terminal runtime
  advance;
- `[w]` starting only after its marker and accepting only positive `ms`/`s`
  durations;
- `[clear]` changing the visible origin only after its marker and retaining
  that origin across `[l]`;
- `[speed ...]` affecting subsequent reveal timing;
- native/Web progression parity;
- session snapshot round-trip, malformed display-map rejection without partial
  mutation, and runtime/presentation consistency;
- stage-scoped Agent observation with advance enabled only for an actionable
  wait.

These are API and runtime-behavior checks. No source gate, source-token search,
or assertion about a symbol remaining in a particular file is part of the
evidence.

## Independent follow-up boundaries

### Mark and dialogue host-event dispatch

`RichTextControl::Mark` and `DialogueHostEvent` markers remain in display-stage
projections, but this slice does not turn reveal traversal into exactly-once
runtime dispatch. A separate feature slice must define typed dispatch,
capability checks, cancellation, snapshot behavior, and native/Web/AWBC parity
for marks, calls, signals, voice, and stage events. It must not infer dispatch
by searching source spellings.

### Duplicate `arcweft-dialogue` model

`crates/arcweft-dialogue/src/lib.rs` still owns a second `DialogueTag` and
`DialogueContent::parse_lossy` model for `[p]`, `[l]`, `[r]`, and `[w]`, while
the compiler path uses syntax/HIR/runtime-plan/render-text types. Remove the
unused duplicate or redesign it as an intentional facade after its consumers
and ownership are audited; do not add conversion wrappers between both models.

### Generic text policy placeholders

`arcweft-layout::TextOverflowPolicy::Page` and
`arcweft-bundle::resource_codec::view::ViewTextRevealPolicy` are separate
provisional generic View contracts. They are not the owner of dialogue logical
pages or the shared player reveal state. Audit their real consumers and either
implement their generic contract or remove the unused variants/bindings; do not
map them to dialogue progression merely because the labels look related.

### Resolved: silent stream/source no-ops

The checked executable-lowering cut now returns structured errors from stream
and source statement lowering and removed `StreamOp::Noop` / `SourceOp::Noop`
from the core model. Evidence and focused negative coverage are recorded in
[Checked executable lowering — 2026-07-10](function-stack-checked-executable-lowering-2026-07-10.md).

### Resolved: lossy executable-expression fallback

Source, stream, ordinary flow return, effect, and host-request executable
positions now use checked lowering and structured diagnostics. The lossy label
lowerer remains only for explicitly non-executable adapter metadata; it is no
longer an error fallback for these executable positions.

### Multiple display frames in one step

`resolve_display_frames` can collect multiple dialogue frames from one runtime
step, while `BundlePresentationSnapshot::update` currently selects only the
last frame. A separate correctness slice must either prove and enforce the
single-frame invariant or preserve every frame with explicit textbox/ordering
semantics. Silently dropping earlier frames is not an acceptable final
contract.

## Structural audit

The final checkout was audited at Jujutsu change `xuromqxp`, based on Git
revision `dc96c4c3`. The canonical report was generated with:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/dialogue-control-playback-2026-07-10
```

The scan covered 2,492 files, including 1,156 Rust files and 587,205 Rust
physical LOC. It reported 0 errors and 151 pre-existing warning-level review
items. Exact metrics for every changed file and the largest workspace files are
in
[`file_metrics.csv`](structure-audits/dialogue-control-playback-2026-07-10/file_metrics.csv);
dependency edges and duplicate public-type candidates are recorded beside it.
Paths under `crates/<name>/` identify the owning crate. Generated files and test
files are classified explicitly in the CSV rather than mixed into production
hotspots.

The main changed ownership boundaries were measured as follows. `Test LOC` is
the exact tail occupied by an inline `#[cfg(test)]` module; zero means tests are
external or the production file has no inline test body.

| Owning crate | File | Bytes | Physical LOC | Test LOC | Main responsibility |
| --- | --- | ---: | ---: | ---: | --- |
| `arcweft-lang-syntax` | `src/ast/dialogue.rs` | 20,526 | 783 | 0 | typed wait and reveal-speed syntax values |
| `arcweft-runtime-plan` | `src/render_text/tag.rs` | 28,200 | 767 | 0 | checked control lowering |
| `arcweft-render-text` | `src/lib.rs` | 57,168 | 1,711 | 463 | resolved rich-text model and display maps |
| `arcweft-render-text` | `src/playback.rs` | 35,408 | 1,017 | 258 | stage derivation and restored-frame validation |
| `arcweft-runtime-driver` | `src/dialogue.rs` | 7,179 | 261 | 0 | occurrence/stage/page identity and transitions |
| `arcweft-runtime-driver` | `src/display.rs` | 56,477 | 1,506 | 754 | presentation snapshot and step input arbitration |
| `arcweft-runtime-driver` | `src/session.rs` | 77,934 | 2,043 | 503 | validated atomic session restore |
| `arcweft-player-scene` | `src/dialogue.rs` | 5,401 | 175 | 48 | shared stage-local visual clock |
| `arcweft-player-scene` | `src/frame.rs` | 17,434 | 485 | 0 | current-stage scene projection |
| `arcweft-render-wgpu` | `src/geometry.rs` | 64,096 | 2,014 | 0 | geometry orchestration after dialogue extraction |
| `arcweft-render-wgpu` | `src/geometry/dialogue.rs` | 17,250 | 515 | 0 | dialogue geometry construction |
| `arcweft-render-wgpu` | `src/geometry/dialogue_timeline.rs` | 14,048 | 455 | 232 | reveal/wait/clear timeline evaluation |
| `arcweft-render-native` | `src/lib.rs` | 50,964 | 1,491 | 0 | native render facade; tests live in `src/tests.rs` |
| `arcweft-render-native` | `src/window_page.rs` | 45,141 | 1,330 | 0 | native page projection from shared stages |
| `arcweft-player-native` | `src/scene_windowed.rs` | 63,720 | 1,727 | 54 | window loop, input routing, and visual-clock save |
| `arcweft-cli` | `src/app/agent/native/player_observation.rs` | 47,901 | 1,347 | 385 | stage-scoped Agent observation |

`geometry.rs` had crossed the 2,500-LOC error threshold before this slice. Its
dialogue construction and timeline responsibilities are now named modules in
the preferred 300–800 LOC range, leaving the orchestrator at 2,014 LOC. Other
warning-level files in the table were not mechanically split where the change
did not establish a clean new responsibility boundary; their warnings remain
visible in
[`violations.md`](structure-audits/dialogue-control-playback-2026-07-10/violations.md).

Workspace dependency fan-in/fan-out for the affected boundary crates is:

| Crate | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-lang-syntax` | 13 | 0 |
| `arcweft-lang-sema` | 8 | 5 |
| `arcweft-runtime-plan` | 8 | 6 |
| `arcweft-render-text` | 16 | 1 |
| `arcweft-runtime-driver` | 6 | 9 |
| `arcweft-player-scene` | 3 | 9 |
| `arcweft-render-wgpu` | 6 | 6 |
| `arcweft-render-native` | 2 | 8 |
| `arcweft-player-native` | 1 | 21 |
| `arcweft-player-web` | 0 | 15 |
| `arcweft-cli` | 0 | 48 |

## Validation record

The final validation cut passed:

- focused syntax/sema/runtime-plan control validation;
- `arcweft-render-text` stage derivation tests;
- runtime-driver page progression, stale-input, exactly-once terminal advance,
  session round-trip, and atomic-restore tests;
- `arcweft-player-scene`, native renderer, native player, Web parity, WGPU
  geometry, and Agent observation checks;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features` (exit 0; only
  previously recorded warnings remain);
- `just test-workspace` (final complete run passed; ignored Tier 2 GPU/exact
  visual tests remain governed by the repository test-execution policy);
- `cargo fmt --all --check`; and
- `git diff --check`.

The first cold workspace-test attempt reached the external 15-minute command
timeout while Cargo was still compiling. A warmed retry exposed one
`arcweft-runtime-host` assertion without enough failure context; the assertion
was made diagnostic, its test binary then passed five consecutive isolated
runs, and a complete warmed suite passed. After the final validation additions,
an unrestricted rerun was interrupted by Windows `os error 1455` because the
page file could not map a large test `.rlib`. No Cargo/rustc process remained.
The identical `just test-workspace` suite was repeated with only
`CARGO_BUILD_JOBS=2` changed and passed completely in 962.7 seconds. No test or
failure is waived as part of this slice.
