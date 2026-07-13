# Ad-hoc audit implementation-ready fixes

This implementation series addresses the findings in
`arcweft-adhoc-audit-4204d259.zip`, audited against revision
`4204d25965129ced50abe82cf5de67d528b483d0`. The package hashes were verified
before implementation. Findings are tested through owned APIs and serialized
behavior; no source gate is used.

## Implemented findings

### AW-AH-005 and AW-AH-006 — RichText builtin ownership

- Made `arcweft-dialogue::rich_text::BuiltinRichTextFx` the closed owner of
  builtin selector, family, phase, and property-schema metadata.
- Replaced the syntax and runtime-plan membership tables with owner lookups and
  typed exhaustive dispatch.
- Classified attribute-free `[.sparkle]` as an effect and lowered it through
  the normal typed Fx path.

### AW-AH-010 — checked physical text bounds

- Added one shared logical-clip-to-physical-bounds constructor in
  `arcweft-glyphon`.
- Validate logical geometry, far-edge addition, raster-scale multiplication,
  outward rounding, and glyphon's signed pixel domain as one operation.
- Propagate structured numeric errors through prepared text and View rendering;
  removed the WGPU zero/clamp pixel fallback.

### AW-AH-014 — shared wheel normalization

- Added host-neutral `WheelDelta`, `LogicalWheelDelta`, and
  `WheelNormalizationPolicy` ownership in `arcweft-player-scene`.
- Native and Web now use the same checked line/physical-pixel conversion and
  route both axes through `precision_scroll`.
- The value `32` is an explicit Arcweft default policy, not an OS or standards
  constant. It preserves the previous interaction scale while removing the two
  backend-local literals, and can be replaced through the checked policy
  constructor.
- Non-finite values, invalid scale factors, and values outside the logical
  `f32` domain are errors rather than implicit zero or saturation.

### AW-AH-016 — required AWFB executable discriminator

- Made the required `ProductManifest.executable_payload` a closed
  `ProductExecutablePayload` enum in the canonical in-memory model; the wire
  representation remains the string `"awbc_v1"`.
- Distinguish a missing field with `MissingProductExecutablePayload`, malformed
  values such as JSON `null` or a duplicate discriminator with `DecodeAwfb`,
  and unsupported strings with `UnsupportedProductExecutablePayload`.
- The typed owner is also the sole owner of the wire spelling; manual Serde
  delegates to `wire_name` / `from_wire_name` rather than duplicating a rename
  literal. A duplicate-aware probe prevents JSON object last-wins behavior.
- Unsupported wire text is retained only in the error diagnostic. The
  canonical model does not preserve an untyped original string.
- The encoder emits only `awbc_v1`; no default, alias, or dual reader remains.

### AW-AH-001, AW-AH-002, and AW-AH-004 — tooling sugar

- Added parser-owned speaker-line surface ranges and CST-owned typed path-root
  ranges, including exact lossless-token boundaries.
- Sugar expansion now consumes those typed ranges. It no longer infers
  `parent::` paths or speaker colons by scanning raw source text.
- Canonicalization and edit application are fallible through structured
  `ToolingError` values; invalid ranges, UTF-8 boundaries, and overlapping edits
  are no longer silently converted into unchanged text.
- A shared `SourceEditOverlay` composes typed path aliases into containing
  speaker, `await?`, and dotted dialogue-default replacements. It consumes an
  alias only after a successful rewrite; partial and unrelated overlaps still
  reach the structured overlap check.
- Focused syntax, tooling, verifier-LSP, and LSP tests cover same-line path
  roots, Unicode, strings/comments/dialogue, nested speakers, containing
  replacements, invalid ranges, and overlapping edits.

## Implementation-ready next cut

### AW-AH-012 — canonical presentation aliases

AW-AH-012 requires no additional design. After this independent request-file
cut, implement it as an independent reviewable change before the typed
presentation ABI work:

- retain one canonical presentation command and argument spelling from the
  accepted language contract;
- delete alternate callee/key recognition from sema and runtime consumers;
- reject removed spellings through normal structured compiler diagnostics;
- leave no runtime alias reader, deprecated spelling, wrapper, or migration
  shim; and
- prove canonical success, removed-spelling rejection, and lack of direct
  runtime alias acceptance through behavior tests rather than source gates.

The resulting canonical semantic identities are fixed substrate for
AW-AH-011/AW-AH-013. That ABI design must not reopen the AW-AH-012 naming cut.

## Design-gated findings

The audit establishes that AW-AH-003, AW-AH-007 through AW-AH-009,
AW-AH-011, AW-AH-013, and AW-AH-015 are real. Their packages explicitly leave
semantic or ABI choices open. They remain completion work, but require
repository-visible design decisions before production migration:

- sema identity supplied to formatting/canonicalization;
- typed/ranged RichText attributes and malformed-value policy;
- character nominal type scope and serialization identity;
- the typed presentation command/AWBC ABI for AW-AH-011 and AW-AH-013;
- vertical-break normalization, quality policy, and evaluation corpus.

Each design-gated boundary now has a standalone request that includes the
accepted finding, established substrate, required decisions, compatibility-free
migration, diagnostics/codecs, and validation contract:

- [AW-AH-003 sema-backed speaker-sugar canonicalization](../reviews/requests/2026-07-14-aw-ah-003-sema-backed-speaker-sugar-canonicalization.md)
- [AW-AH-007/008 typed RichText attribute validation](../reviews/requests/2026-07-14-aw-ah-007-008-typed-rich-text-attribute-validation.md)
- [AW-AH-009 character nominal type identity](../reviews/requests/2026-07-14-aw-ah-009-character-nominal-type-identity.md)
- [AW-AH-011/AW-AH-013 typed presentation command ABI](../reviews/requests/2026-07-14-aw-ah-011-and-013-typed-presentation-command-abi.md)
- [AW-AH-015 vertical-break quality policy](../reviews/requests/2026-07-14-aw-ah-015-vertical-break-quality-policy.md)

No provisional compatibility layer will be introduced while those designs are
completed.

## Validation

Completed focused evidence:

- RichText owner/checks: `arcweft-dialogue` 7 tests,
  `arcweft-lang-syntax` 78 library tests, and `arcweft-runtime-plan` 233 tests;
  related all-target/all-feature Clippy passed with `-D warnings`.
- Physical text bounds: `arcweft-glyphon` 15 tests, `shared_text_layout` 6
  tests, and `arcweft-render-wgpu` 64 tests; related check and Clippy passed.
- Wheel normalization: 5 focused `arcweft-player-scene` tests passed; Native,
  Web, and scene Clippy passed with `-D warnings`.
- AWFB product manifest: 10 focused tests and the full 89-test bundle unit
  suite passed; bundle Clippy passed with `-D warnings`.
- Tooling passed 54 tests; LSP passed 95 tests and verifier-LSP passed 16 tests
  before the final overlay generalization. The final tooling overlay change was
  revalidated by its 54-test suite, all-target/all-feature check, and Clippy
  with `-D warnings`.
- The integrated 12-crate all-target/all-feature check passed. Workspace Clippy
  passed. `just test-fast` passed its 183 / 36 / 9 / 64 / 15-test suites.
- `just test-workspace` completed the non-CLI workspace lib/test phase without
  a test failure. Its following CLI build stopped while creating an
  `arcweft-bundle` archive because the drive ran out of space (`os error 112`),
  before the CLI tests began. This is a remaining validation gap, not an
  assertion failure.
- A preliminary combined non-incremental focused invocation also hit its
  242.6-second command timeout and produced a broken-pipe panic when the test
  harness was killed; the same AWFB cases passed 10/10 when rerun alone.
- The canonical structural audit scanned 2,687 files / 1,282 Rust files and
  reported 0 errors / 126 existing warnings. Exact metrics and dependency
  edges are recorded under
  `docs/implementation/structure-audits/adhoc-audit-implementation-ready-2026-07-13/`.
- Before the push cut, comparison/visual/structural outputs totaling 93.11 MB
  were temporarily preserved, `cargo clean` removed 80,974 generated files /
  100.3 GiB, and the preserved outputs were restored. Drive free space after
  cleanup was 264.89 GiB.

Formatting and `git diff --check` passed. No implementation TODO remains in the
findings listed under **Implemented findings**. The design-gated findings above
remain separate completion work.
