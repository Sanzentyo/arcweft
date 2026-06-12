# Native Capture Fixtures

This directory contains intentionally checked-in native-renderer image fixtures.

- `vertical_tutr_golden.arcw` exercises `vertical_rl` rich text with UAX #50
  `Tu` and `Tr` glyph forms (`。` and `ー`) plus mixed Latin and
  text-combine-upright digits.
- `vertical_tutr_golden.png` is the Windows native `arcw agent observe`
  framebuffer golden generated from that source with the `MS Mincho` font.

Regenerate the PNG on a Windows machine with stable native fonts:

```bash
cargo run -p arcweft-cli -- agent observe tests/fixtures/native_capture/vertical_tutr_golden.arcw --json --image png --out tests/fixtures/native_capture/vertical_tutr_golden.png --mode drain --steps 4 --max-ops 64
```

The CLI test compares a fresh candidate capture against the checked-in PNG with
`imq` when both Windows and the `imq` binary are available. A non-ignored CLI
fixture-integrity test also checks that the checked-in source keeps the vertical
Tu/Tr and text-combine coverage and that the PNG remains a 1280x720 Agent
capture; exact pixel parity stays in the Tier2 `imq` visual regression.
