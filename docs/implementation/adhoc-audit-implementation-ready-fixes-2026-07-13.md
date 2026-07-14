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

### AW-AH-012 — canonical presentation aliases

- Retained `image`, `bg`, and `player_viewport` as the only presentation
  command spellings. Their canonical keys are `width` / `height` / `fit`,
  `alignment.x` / `alignment.y`, and `playback.start` /
  `playback.paused_at` / `playback.local_time`.
- Removed alternate callee and key recognition from semantic checking, project
  indexing, bundle asset discovery, image declaration projection, and the
  runtime driver. Launch TOML, Agent JSON, capture scopes, and other unrelated
  boundaries that use similar words were not changed. In particular, the
  separate `viewport()` Agent intrinsic remains valid; the runtime presentation
  dispatcher simply no longer treats `viewport` as `player_viewport`.
- Added the general machine-readable
  `TypeCheckErrorKind::UnknownPresentationArgument` diagnostic for unknown
  named arguments on the affected commands. Removed dotted callees use the
  ordinary unresolved-call diagnostic, while spellings that are not legal
  argument grammar fail with `syntax.parse`.
- Direct runtime calls containing only removed command or viewport-key
  spellings are no-ops. Removed image keys no longer change an existing image
  object, and bundle tooling no longer discovers or projects them.
- Image declarations still preserve arbitrary unknown fields as open source
  metadata, but the removed keys no longer have projection semantics. Closing
  and typing the complete declaration field contract belongs to the
  AW-AH-011/AW-AH-013 ABI redesign; it is not retained as an alias reader here.
- No production runtime alias reader, deprecated spelling, wrapper, migration
  shim, or source gate remains. All removal evidence is exercised through
  parser, semantic, bundle, or runtime behavior.

These canonical semantic identities are fixed substrate for
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

## AW-AH-012 structural audit

The canonical audit ran at Jujutsu change `zqskstvu`. It scanned 2,699 files /
1,289 Rust files / 629,584 Rust physical LOC and reported 0 errors / 126
existing warnings. The full workspace rankings, dependency edges, and public
type inventory are stored in the
[AW-AH-012 structural audit](structure-audits/aw-ah-012-canonical-presentation-spellings/violations.md).

| Changed Rust file | Classification and responsibility | Bytes | Physical LOC |
| --- | --- | ---: | ---: |
| `crates/arcweft-cli/src/app/bundle.rs` | production; bundle assembly, image metadata projection, and static asset discovery | 70,318 | 1,971 |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | unit test; bundle image projection and asset-discovery behavior | 79,322 | 2,486 |
| `crates/arcweft-lang-sema/src/checker/presentation.rs` | production; canonical presentation call and argument checking | 23,256 | 572 |
| `crates/arcweft-lang-sema/src/diagnostics.rs` | production; semantic error kinds, messages, and stable codes | 43,860 | 1,231 |
| `crates/arcweft-lang-sema/src/project_index/entities.rs` | production; typed entity and Agent-action indexing | 33,401 | 942 |
| `crates/arcweft-lang-sema/src/project_index/tests.rs` | unit test; canonical and removed image-call indexing behavior | 13,923 | 455 |
| `crates/arcweft-lang-sema/src/tests/mod.rs` | unit-test facade | 455 | 26 |
| `crates/arcweft-lang-sema/src/tests/presentation.rs` | unit test; canonical and removed presentation spelling behavior | 6,134 | 143 |
| `crates/arcweft-runtime-driver/src/display.rs` | production with embedded unit tests; snapshot projection and viewport/image command consumption | 61,455 | 1,631 |

No Cargo dependency changed. The recorded package fan-out / fan-in counts are
9 / 8 for `arcweft-lang-sema`, 13 / 6 for `arcweft-runtime-driver`, and 65 / 0
for the application-only `arcweft-cli`. Existing size warnings remain for
`bundle.rs`, `diagnostics.rs`, and `display.rs`. `display.rs` consists of 740
production LOC and 891 embedded-test LOC; it remains below the error threshold,
and AW-AH-011/AW-AH-013 is the planned ownership redesign rather than splitting
the string-command consumer during this alias-only cut.

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
- AW-AH-012 passed all 517 semantic library tests, all 117 runtime-driver
  tests, and all 193 CLI library tests. The affected sema/runtime/CLI
  all-target/all-feature check passed, as did workspace all-target/all-feature
  Clippy with `-D warnings`.
- The integrated 12-crate all-target/all-feature check passed. Workspace Clippy
  passed. `just test-fast` passed its 183 / 36 / 9 / 64 / 15-test suites.
- The earlier `just test-workspace` run stopped before CLI tests when the drive
  ran out of space (`os error 112`). The current AW-AH-012 cut reran the full
  command after cleanup and it passed, resolving that validation gap.
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
- Tier 2 MCP stdio, broad Agent observe, exact visual-golden, and doc-test
  suites were intentionally not run for AW-AH-012 because this cut changes no
  MCP, capture, renderer, image output, or Rust documentation contract.

Formatting and `git diff --check` passed. No implementation TODO remains in the
findings listed under **Implemented findings**. The design-gated findings above
remain separate completion work.
