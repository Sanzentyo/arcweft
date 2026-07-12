# Unified Text / Dialogue View / Fx implementation status — 2026-07-12

Design source:

- external directive `arcweft-unified-text-textbox-view-implementation-directive.md`;
- user-supplied final numeric, time, transform, target, bundle, and save
  decisions;
- [final repository design](../design/unified-text-dialogue-view-fx-final-design-2026-07-12.md).

Baseline revision: `d934189ba1e414bfa23f7792658e69fd8c60d714`.

## Current status

The implementation is complete. Commit
`29f0dded1039f0c7dc2fbbb041c449e400df2424` (`Finalize unified text and typed
dialogue Views`) is the completed product cut. Commit `4f733887` records its
integration evidence.

All ordinary display text now converges on:

```text
ResolvedTextDocument
  -> TextLayout
  -> PreparedTextItem
  -> ViewPrimitive::Text
  -> SharedRenderer
```

Dialogue presentation is a persistent authored View mount. There is no public
`textbox` declaration, entity kind, reference family, line option, manifest,
runtime domain object, renderer primitive, compatibility alias, dual reader,
or removed-spelling parser branch. Any historical mention of a TextBox below
describes a discarded intermediate implementation and is not a supported
contract.

## Final dialogue View contract

Canonical source uses `pub dialogue defaults { view = @view.Name }`. Named
profiles use direct IDs such as `@dialogue.mobile`. The ordinary defaults
declaration omits a redundant defaults ID. Canonical modules use dotted paths,
such as `mod game.opening`; `mod game::opening` is rejected with a structured
diagnostic.

`DialogueView` is a public standard-prelude nominal record visible to semantic
analysis and LSP. The `#[dialogue_view]` attribute applies the same exact,
closed six-field projection contract to a custom nominal record:

| Field | Type |
| --- | --- |
| `speaker` | `String` |
| `content` | `DialogueContent` |
| `occurrence` | `DialogueOccurrenceId` |
| `stage` | `DialogueStage` |
| `reveal` | `DialogueReveal` |
| `primary_action` | `DialogueAction` |

Missing, additional, duplicate, or wrongly typed role fields are structured
errors. The attribute does not create an open property bag or a second View
type system.

The reserved linked resource `std.view.dialogue` supplies the standard minimal
presentation. Omitted and explicit project selections use the same evaluator,
mount, prepared-text, renderer, interaction, accessibility, Agent, capture,
and save/load path. Authored identity is `DialogueViewDefinition`; runtime
target identity is `DialoguePresentationId`; each occurrence retains a
persistent `ViewMountId`. Multiple targets that share one definition remain
independent.

Dynamic `Text` and `RichText` retain authored `x`, `y`, `width`, and `height`.
`Panel` and `Box` nodes lower to authored surfaces. Their surface union supplies
root render, hit, focus, accessibility, avoidance, Agent, and capture bounds.
No renderer-local dialogue layout fallback remains. Samples and positive
fixtures use `style`, `view`, `DialogueView`, the `view` option, and
`wrap = container`.

## Unified text and Fx result

The shared shaped path retains horizontal and vertical text, ruby,
text-combine, JLREQ composition, selection, IME, reveal, and typed Fx. Fx
definitions, parameter programs, sampler IR, renderer resources, instance
state, and diagnostics use shared typed contracts rather than native-only
execution or renderer-local formula copies.

Glyph Fx sampling rebases `ctx.ordinal` to the first logical glyph of each
retained Fx application. A later span therefore does not inherit the entire
document's glyph index. Frame observation reports effective glyph opacity
after local paint opacity, resolved transform opacity, and the complete mask
chain. Effective-zero glyphs are not reported as visible or included in
visible ranges.

Save/load retains stable dialogue occurrence, presentation, View mount,
activation time, deterministic seed, reactive parameters, and nested child
ordinal state. Restore validates the store, output, and retained-mount
correspondence before mutation and rejects tampered or orphaned records
atomically.

## Final visual and semantic evidence

The final `just unified-text-visual-parity` packet is generated under
`target/unified-text-visual-parity/`. Its `verification-summary.json` passes.
All eight checkpoints are pixel-exact between Native and Web:

- vertical-RL and vertical-LR, including ruby and text-combine;
- JLREQ loose and strict composition;
- source-defined Fx at 4,000 ms and quantized 4,512 ms;
- typewriter reveal at 20,000 ms and quantized 20,512 ms.

The verifier records a 16 ms logical-clock quantum. The intended semantic and
temporal differences are non-zero and identical on both backends:

| Comparison | Native MSE | Web MSE |
| --- | ---: | ---: |
| JLREQ loose vs strict | 0.0012708181137564513 | 0.0012708181137564513 |
| Fx 4,000 ms vs 4,512 ms | 0.00029181310643758816 | 0.00029181310643758816 |
| reveal 20,000 ms vs 20,512 ms | 0.00033434599967655405 | 0.00033434599967655405 |

The scoped vertical-RL, vertical-LR, and Fx packets retain color, object-ID,
and mask attachments. The transparent full-panel primary action keeps its
authored hit geometry while its empty label emits no prepared-text item.

For `samples/unified-text-visual-parity/main.arcw`, Agent observation reports
the authored root at `x=57, y=460, width=1166, height=203`; the vertical content
run is `x=958, y=518, width=237, height=142`. The standard View integration
test also exercises vertical-RL text and ruby through the same prepared-text
path. No checked-in golden was overwritten to obtain this result.

## Final validation

The completed product checkout passed:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
just test-workspace
just unified-text-visual-parity
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-dialogue-view-final-2026-07-13
```

Results:

- `just test-fast`: 5 suites, 307 passed, 0 failed, 0 ignored;
- `just test-workspace`: 234 suites, 3,259 passed, 0 failed, 16 intentionally
  ignored by the normal non-Tier-2 route;
- visual parity: 8 Native/Web checkpoints, pixel-exact backend pairs, with the
  non-zero JLREQ, Fx, and reveal differences above;
- structural audit: 2,651 scanned files, 1,255 Rust files, 616,763 physical
  Rust LOC, 91 manifests, 0 errors, and 128 tracked warnings; the product cut
  introduced no warning- or error-threshold crossing.

Final validation exposed and closed three integration defects rather than
waiving them: missing typed shader resources retain the `missing_provider`
diagnostic code; dialogue restore rejects incomplete store/output/mount
correspondences atomically; and the obsolete `face=` negative fixture was
removed with its name-specific parser branch.

On Windows the test profile used `CARGO_PROFILE_TEST_DEBUG=0` and
`CARGO_INCREMENTAL=0` after the default linker reached its PDB capacity and the
incremental cache exhausted the workspace drive. This changed disposable debug
artifacts, not code, features, test selection, or runtime behavior.

### Post-completion View test ownership cleanup — 2026-07-13

The `arcweft-view` facade now contains only its intentional public surface and
error contract. Root-level tests moved into responsibility modules for
fragment/layout, display/frame, images, entity/reactive/registry, and
semantics. Text-field edit and policy/geometry tests likewise moved out of the
production module. This changed no public API or behavior: all 60
`arcweft-view` unit and integration tests pass, and all-target/all-feature
Clippy passes with warnings denied.

The canonical structural audit measures `lib.rs` at 152 physical LOC and
`text_field.rs` at 1,194 physical LOC. The former `SIZE002` facade warning and
the latter `SIZE001` plus `TEST001` warnings are gone; `arcweft-view` has no
remaining structural warning.

### Typed bundle hardening and vertical Style evidence — 2026-07-13

The unpublished View bundle contract now stores a required closed
`ViewParameterRole::{Value, Dialogue}` and a required
`ViewTextSurface::{Text, RichText}`. CLI lowering derives the Dialogue role
from the semantic `DialogueViewModelRegistry`; the bundle never guesses from a
nominal spelling. Codec, complete-bundle, runtime-construction, and evaluator
validation reject a missing or wrong role, a speaker/content surface mismatch,
and a primary action wired to a scalar parameter. No defaulted field,
compatibility reader, alias, or format-version bump was added.

A dedicated integration fixture compiles an exact custom six-field
`#[dialogue_view] StoryDialogue` through parse, HIR/sema, CLI sidecar lowering,
AWFB decode, and `BundleViewRuntime` evaluation. Hand-built and tampered bundle
tests cover the negative role, surface, and action cases.

The scene adapter no longer applies a renderer-local 4/5 speaker font-size and
line-height reduction or a fixed bold weight after resolving a View part.
Authored Style metrics and weight now reach the shared prepared-text path
unchanged; only the speaker label's horizontal flow remains semantic. The
standard View test observes its authored 25 px / 34 px metrics directly.

`samples/vertical-writing-style/main.arcw` is the visible Style reference. Its
authored View and Style own panel geometry, font family, colors, and speaker
metrics, while typed RichText selectors own `vertical_rl` / `vertical_lr`,
JLREQ, ruby placement, sideways Latin, and text-combine-upright. The
`just vertical-writing-style-sample` gate captures four shared-WGPU PNGs,
checks all four panel pixels and structured text metadata, proves loose/strict
JLREQ output differs, and repeats vertical-RL pixel-exactly. The repeat SHA-256
is `47E518B9ED031D8B18A00B6FF5F7BF37A6D1E5C8DE977E9364B13AAF537DB561`.

The former checked-in vertical capture references represented the removed
legacy dialogue geometry. After retaining their failed IMQ packets and
visually inspecting the candidates, the four exact references were manually
promoted to the final authored Dialogue View output. `just
test-visual-golden` then passed both smoke tests and all four exact IMQ tests.
The checked-in `web/demo.awfb` was likewise regenerated from `web/arcw.toml`
because the corrected unpublished View resource fields are mandatory.

Validation for this hardening cut passed workspace check, workspace Clippy
with warnings denied, 307 fast tests, the normal workspace suite, the vertical
Style gate, eight pixel-exact Native/Web unified-text checkpoints, and the
exact vertical golden gate. The structural audit under
`structure-audits/dialogue-view-hardening-vertical-style-2026-07-13/` reports
2,664 files, 1,264 Rust files, 617,309 Rust LOC, 0 errors, and 125 warnings—three
fewer warnings than the completed product cut.

The full Tier 2 aggregate was also executed, but its independent ignored MCP
stdio harness remains stale: 9 of 25 tests pass and 16 expect raw resource
URIs/semantic IDs or direct image payloads where the current strict
content-policy contract returns opaque moderated resources. Its separate slow
Agent-observe matrix also expects legacy `frame/0` and 96/548/1088x124 dialogue
geometry instead of the authored View's frame 3 and 57/460/1166x203 bounds.
Those harness expectations need a dedicated policy-aware, response-driven
rewrite; production policy was not weakened and the failures were not hidden
with test-only bypasses.

## Acceptance matrix

| Cut | Result |
| --- | --- |
| 1. migration witnesses and resolved document | complete |
| 2. typed Fx IR, evaluator, bundle, symbol, and save contracts | complete |
| 3. shaped shared layout and glyphon engine | complete |
| 4. prepared batch and ordinary text producers | complete |
| 5. RichText, reveal, shared Fx, and native registry removal | complete |
| 6. direct View text order and executable per-mount View | complete |
| 7. shared capture and prepared-layout Agent geometry | complete |
| 8. authored persistent dialogue View and dedicated path removal | complete |
| 9. visual/workspace validation and structural audit | complete |

There are no deferred acceptance items from the supplied implementation
directive.

## Historical appendix

The migration began at baseline revision
`d934189ba1e414bfa23f7792658e69fd8c60d714`. Baseline experiments established
that the then-current native dialogue path had already regressed vertical
rendering and that Fx/reveal still depended on a separate presentation route.
The baseline packet under `target/unified-text-baseline-d934189b/` was retained
only as a migration witness. It was never the expected final output.

The implementation proceeded through nine compiling cuts: canonical resolved
documents; typed Fx contracts; shaped layout; prepared batches; shared
RichText/Fx composition; executable View mounts; shared capture; persistent
authored dialogue Views; and final visual/workspace validation. Some
intermediate working changes temporarily contained a dedicated TextBox store,
Rust-created standard panel, legacy renderer registries, or notes saying a cut
“remains open.” Each was removed or superseded before the completed product
commit. None is a compatibility promise or current TODO.

Detailed per-cut measurements remain available in the immutable audit
directories under `docs/implementation/structure-audits/`, including:

- `unified-text-final-design-2026-07-12`;
- `unified-text-fx-contracts-2026-07-12`;
- `unified-text-shaped-layout-2026-07-12`;
- `unified-text-prepared-batch-2026-07-12`;
- `unified-text-direct-view-text-2026-07-12`;
- `unified-text-layer-capture-identity-2026-07-12`;
- `unified-text-dialogue-view-final-2026-07-13`.

Those records are historical implementation evidence. The current contract is
defined by the final design and the completed status sections above.

## Non-goals

Typst `TypesetBlock` remains a separate document-rendering system and is not an
ordinary player text producer covered by this unification.
