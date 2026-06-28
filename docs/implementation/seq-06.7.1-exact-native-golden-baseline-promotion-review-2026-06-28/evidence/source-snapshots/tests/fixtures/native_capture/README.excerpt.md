# Native Capture Fixtures excerpt

Source: `tests/fixtures/native_capture/README.md` on inspected main.

- `vertical_tutr_golden.arcw` exercises `vertical_rl` rich text with UAX #50 `Tu` and `Tr` glyph forms plus mixed Latin and text-combine-upright digits.
- `vertical_tutr_golden.png` is the Windows native `arcw agent observe` framebuffer golden generated from that source with the `MS Mincho` font.
- Exact visual-golden tests compare fresh candidate captures against checked-in PNGs with `imq` only when both Windows and `imq` are available.
- A milestone or CI handoff that needs publishable evidence must run `just native-visual-artifacts`; it writes fresh candidate PNGs, observe JSON, `imq` JSON reports, and `exact-native-golden.environment.json`.
- Do not overwrite checked-in PNGs from a refresh command directly. A baseline replacement requires a complete candidate packet, human visual review, recorded before/after metrics, and a documented reason that the candidate is the intended renderer output.
