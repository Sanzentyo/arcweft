# Native Capture Fixtures

This directory contains intentionally checked-in native-renderer image fixtures.

- `vertical_tutr_golden.arcw` exercises `vertical_rl` rich text with UAX #50
  `Tu` and `Tr` glyph forms (`。` and `ー`) plus mixed Latin and
  text-combine-upright digits.
- `vertical_jlreq_preset_loose_golden.arcw` and
  `vertical_jlreq_preset_normal_golden.arcw` exercise the same
  `天地。」人山川海。『火水木` closing/opening composition with different JLREQ
  strictness presets. UAX #14 hard constraints determine the permitted break
  candidates before the `balanced_v1` preset preferences are applied; the
  fixture includes permitted closing/opening opportunities at which `loose`
  and `normal` must choose distinct column plans.
- `vertical_lr_ruby_text_combine_golden.arcw` exercises `vertical_lr` rich text
  with a ruby annotation, a hard line break, sideways Latin, upright punctuation,
  and a 4-digit text-combine-upright cluster.
- `vertical_goal_clear_smoke.arcw` is an all-in-one Agent/native smoke source
  covering `vertical_rl`, `vertical_lr`, ruby on both physical sides,
  text-combine-upright digits, sideways Latin, and capture-time controlled
  typewriter visibility. Its PNG and raw crops are generated in tests rather
  than checked in as stable goldens.
- `unified_text_effects_migration_baseline.arcw` is the temporary migration
  witness for fixed-time wave, shake, jitter, typewriter, spin, pulse, shader,
  post-process, host effect, source Fx transform, and vertical-effect output.
  It deliberately uses only resolvable resources at the baseline revision;
  generated images live in the migration evidence packet until the shared path
  replaces them.
- `unified_text_reveal_vertical_migration_baseline.arcw` keeps the reveal,
  stacked motion, and vertical-effect rows inside the visible two-line region
  of the standard authored dialogue View so fixed-time captures contain actual pixels
  for those features rather than clipped authored content.
- `vertical_tutr_golden.png` is the Windows native `arcw agent observe`
  framebuffer golden generated from that source with the `MS Mincho` font.
  The JLREQ preset and `vertical_lr` ruby/text-combine PNGs are generated the
  same way from their matching sources.

Regenerate the PNG on a Windows machine with stable native fonts:

```bash
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_tutr_golden.arcw --entry entry.vertical_tutr_golden --json --image png --out tests/fixtures/native_capture/vertical_tutr_golden.png --mode drain --steps 4 --max-ops 64
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.arcw --entry entry.vertical_jlreq_preset_loose_golden --json --image png --out tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.png --mode drain --steps 4 --max-ops 64
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.arcw --entry entry.vertical_jlreq_preset_normal_golden --json --image png --out tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.png --mode drain --steps 4 --max-ops 64
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.arcw --entry entry.vertical_lr_ruby_text_combine_golden --json --image png --out tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.png --mode drain --steps 4 --max-ops 64
```

The CLI `visual_smoke` tests are the default non-exact validation tier for this
directory. They generate temporary viewport, selected layer, selected object,
object-id, mask, and overflow/wrap captures; assert dimensions, non-empty image
content, crop bounds, and seq06.5 `selected_capture` metadata; and avoid exact
pixel comparison. The exact visual-golden test compares fresh candidate captures
against the checked-in PNGs with `imq` only when both Windows and the `imq`
binary are available. A non-ignored CLI fixture-integrity test also checks that
the checked-in sources keep the intended vertical coverage, that every PNG
remains a 1280x720 Agent capture, and that the loose/normal preset PNGs are
distinct, preserving evidence that the two presets choose different accepted
plans without overriding UAX #14 prohibitions. The goal-clear smoke fixture is
validated by non-ignored CLI tests
that generate a temporary native PNG; color, mask, and object-id raw crops from
the same text-combine/typewriter object; and mask / object-id raw crops from the
vertical ruby objects on both physical sides.
Tier2 `imq` visual regression uses bounded full-reference MSE/MAE drift rather
than exact-zero pixel parity because the native text path can produce tiny
antialiasing differences across otherwise valid Windows renderer/font
environments. For a milestone or CI handoff that needs publishable evidence, run
`just native-visual-artifacts`; it writes fresh candidate PNGs, observe JSON,
`imq` JSON reports, and `exact-native-golden.environment.json` to
`target/arcweft-native-capture-artifacts/`. Exact visual failures report the
fixture id, checked-in reference path, candidate path, observe JSON path, metric
JSON path, and environment fingerprint path.

Exact native golden review uses these status classes:

- `expected_skip` for local/non-Windows discovery runs where exact evidence is
  not required;
- `environment_not_pinned` for required jobs that did not assert the pinned
  environment contract;
- `environment_blocker` for missing `imq`, missing pinned `MS Mincho`, or an
  unsupported backend;
- `baseline_drift` when dimensions match but MSE/MAE exceed the fixture bounds;
- `hard_visual_regression` for capture failure, `imq` failure, dimension
  mismatch, malformed PNGs, or missing artifacts.

Do not overwrite checked-in PNGs from a refresh command directly. A baseline
replacement requires a complete candidate packet, human visual review, recorded
before/after metrics, and a documented reason that the candidate is the intended
renderer output.
