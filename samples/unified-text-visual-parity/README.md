# Unified Text Visual Parity

This sample is the deterministic project-font fixture for unified Text,
authored dialogue View, Fx, and capture. Five sequential dialogue pages cover:

- vertical-rl UAX #50 punctuation, text-combine-upright, and sideways Latin;
- vertical-lr ruby-under, inter-character ruby, text-combine-upright, and
  sideways Latin;
- loose and strict JLREQ composition of the same closing/opening pair in
  separate frames at the same constrained large glyph size; and
- a source-defined glyph transform plus a delayed typewriter reveal at fixed
  logical times.

`pub style unified_text_panel` declares the text-style contribution.
`pub view UnifiedTextPanel(dialogue: DialogueView)` explicitly places the
speaker and rich dialogue content, and `pub dialogue defaults` selects it.
Dialogue has no separate presentation entity or renderer path: every active
target is a persistent authored View mount using the ordinary prepared-text
batch and `ViewPrimitive::Text` path.

Run `just unified-text-visual-parity` to build the bundle and collect the
Native/headless offscreen and WebGPU parity packet under
`target/unified-text-visual-parity/`. The command never overwrites a checked-in
native-system-font golden.

The harness stops the runtime clock at each target page's activation tick and
then advances the same 16 ms logical quanta on Native and Web. The final
verifier requires pixel-exact cross-backend frames, distinct loose/strict JLREQ
images, moving source-defined Fx, one-glyph reveal progression, and color,
mask, and object-ID attachments for representative dialogue-View content.
