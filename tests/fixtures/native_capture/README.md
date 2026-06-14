# Native Capture Fixtures

This directory contains intentionally checked-in native-renderer image fixtures.

- `vertical_tutr_golden.arcw` exercises `vertical_rl` rich text with UAX #50
  `Tu` and `Tr` glyph forms (`。` and `ー`) plus mixed Latin and
  text-combine-upright digits.
- `vertical_jlreq_preset_loose_golden.arcw` and
  `vertical_jlreq_preset_normal_golden.arcw` exercise the same repeated-leader
  paragraph with different JLREQ strictness presets, so visual regression can
  catch preset-specific column-plan drift.
- `vertical_lr_ruby_text_combine_golden.arcw` exercises `vertical_lr` rich text
  with a ruby annotation, a hard line break, sideways Latin, upright punctuation,
  and a 4-digit text-combine-upright cluster.
- `vertical_goal_clear_smoke.arcw` is an all-in-one Agent/native smoke source
  covering `vertical_rl`, `vertical_lr`, ruby on both physical sides,
  text-combine-upright digits, sideways Latin, and capture-time controlled
  typewriter visibility. Its PNG and raw crops are generated in tests rather
  than checked in as stable goldens.
- `vertical_tutr_golden.png` is the Windows native `arcw agent observe`
  framebuffer golden generated from that source with the `MS Mincho` font.
  The JLREQ preset and `vertical_lr` ruby/text-combine PNGs are generated the
  same way from their matching sources.

Regenerate the PNG on a Windows machine with stable native fonts:

```bash
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_tutr_golden.arcw --json --image png --out tests/fixtures/native_capture/vertical_tutr_golden.png --mode drain --steps 4 --max-ops 64
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.arcw --json --image png --out tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.png --mode drain --steps 4 --max-ops 64
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.arcw --json --image png --out tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.png --mode drain --steps 4 --max-ops 64
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.arcw --json --image png --out tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.png --mode drain --steps 4 --max-ops 64
```

The CLI test compares fresh candidate captures against the checked-in PNGs with
`imq` when both Windows and the `imq` binary are available. A non-ignored CLI
fixture-integrity test also checks that the checked-in sources keep the intended
vertical coverage, that every PNG remains a 1280x720 Agent capture, and that the
loose/normal preset PNGs are distinct. The goal-clear smoke fixture is validated
by non-ignored CLI tests that generate a temporary native PNG; color, mask, and
object-id raw crops from the same text-combine/typewriter object; and mask /
object-id raw crops from the vertical ruby objects on both physical sides.
Tier2 `imq` visual regression uses bounded full-reference MSE/MAE drift rather
than exact-zero pixel parity because the native text path can produce tiny
antialiasing differences across otherwise valid Windows renderer/font
environments.
