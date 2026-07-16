# Native environment Style condition implementation

## Package and checkout

- Sequence: `seq-06.11d.4.2.1`
- Package:
  `arcweft-seq-06.11d.4.2.1-native-environment-style-condition-production-reconciliation-final-contract.zip`
- Package SHA-256:
  `06054d7a8e70a505680aa2661d21b033c1c5c24799acf0cff439e0855687e793`
- Original package implementation base: `1aa5ad6d395e` (`plovynom`)
- Integrated base: `0865651127dd` (`yrtlmnws`)
- Jujutsu change: `pysvkrvuzyxp`

The sufficiently designed package cuts are implemented through the native
syntax-to-player pipeline. CSS is not reintroduced. The implementation adds no
compatibility shim, dual reader, source gate, removed-syntax recognizer, or
environment-only source identity.

## Implemented contract

### Checked presentation and View ownership

- `arcweft-presentation` owns checked text scale, closed environment fields and
  values, strict Serde decoding, overrides, field sets, equality-aware global
  and field revisions, and snapshots. Locale remains outside Style identity.
- `arcweft-view` owns checked clauses and conditions, canonical field ordering,
  direct rule guards, zero-specificity matching, short-circuit field usage,
  field-local cache keys, inherited identity, and projection-only palette
  invalidation.
- The existing container comparison owner remains the sole comparison enum;
  no environment-local duplicate was introduced.

### Native language pipeline

- `arcweft-lang-syntax` retains explicit environment nodes, exact ranges,
  closed value token classes, arbitrarily long percentages without integer
  conversion, and delimiter-aware recovery.
- `arcweft-lang-hir` carries typed clauses, operand spelling, source ranges,
  and typed recovery rather than raw expressions.
- `arcweft-lang-sema` validates the closed value and comparison matrices,
  checked text-scale precision/range, duplicate fields across an effective
  nested path, and suppresses executable output for invalid paths.
- `arcweft-compiler` flattens each valid effective path into one checked guard
  while retaining the condition, clause, and guarded-rule provenance that the
  current source model can represent.

### Product, codec, and merge

- Bundle View product codecs round-trip strict checked environment conditions
  and reject malformed, noncanonical, duplicate, over-limit, wrong-kind, and
  out-of-range input before an owned condition exists.
- Product validation checks source owner, extent, UTF-8 boundaries, available
  condition/clause containment, and guarded-rule provenance through the
  canonical final source index.
- Merge performs source remapping and condition/clause budget validation before
  mutation, preserving atomic failure and deterministic ordering.
- Large codec and model owners were decomposed into responsibility modules
  before adding the new wire surface.

### Session, player, and host delivery

- `arcweft-runtime-driver` owns provider/theme/session override precedence and
  one equality-aware transaction for provider updates, override removal,
  theme replacement, clear-to-default, revision overflow, and hot-swap
  rollback.
- `arcweft-player-scene` consumes the session's exact changed-field set,
  retains field revisions in prepared-work stamps, distinguishes selection
  from projection invalidation, and keeps unrelated prepared work reusable.
- `arcweft-player-native` owns bounded FIFO ingress, reserved sequence order,
  patch-before-environment application at one frame boundary, typed receipts,
  overflow/capacity/shutdown behavior, and redraw coalescing.
- Native window orchestration was split into frame-cycle and input-cycle
  modules instead of extending the former monolith.

### Web, headless, and tooling

- `arcweft-player-web` retains players by monotonic handle identity, owns one
  shared `winit` event loop for multiple players, decodes the complete strict
  JS snapshot before borrowing a player, rejects reentrant and use-after-close
  updates deterministically, and keeps existing unit-returning start exports.
- The real-browser harness covers strict decode, two-player independence,
  duplicate canvas ownership, reentry, same-value/change reports, stop/drop,
  and closed-player behavior.
- Native, Web, and headless observation use the same checked environment and
  scene semantics.
- `arcweft-tooling` supplies typed format, completion, hover, semantic-span,
  action, navigation, and edit-invalidation results. `arcweft-lsp` only
  projects those results and does not recompute language rules.

## Integration reconciliations

- The implementation was rebased onto `0865651127dd` after the scaffold
  declaration removal, exact launch-profile topology, and private proof
  declaration cuts. The environment Style parser does not restore removed
  declarations or shadow the project-loader topology owner; the post-rebase
  syntax-through-LSP suites and workspace gates pass.
- `arcweft-adapter-context` now imports `thiserror::Error` for the unconditional
  `AdapterRegistryError` definition instead of hiding that import behind its
  `sema` feature. This fixes the default-feature native build exposed by the
  integrated checkout without adding a new boundary or compatibility layer.

- The logical-axis provider regression now treats an unused environment-field
  change as a field-local cache hit, while retaining the separate d.4.1 axis
  identity.
- The duplicate text-block bundle fixture removes the cloned action's exported
  part before insertion, so it exercises the intended text-block duplicate
  instead of failing earlier on the exported-part invariant.
- Three runtime View fixtures that emitted named text blocks without declaring
  the mandatory canonical inventory now provide the corresponding owner,
  source, and bounds records. The product validation remains strict; the
  focused runtime suite passes 15/15 after the fixture repair.
- Launch-only profiles are no longer mistaken for whole-project source-root
  selections merely because they carry a manifest. `SourceSelection` now
  exposes `project_manifest()` for the only path allowed to enumerate a source
  root. Profile runs compile their selected source while retaining manifest
  package/resource context.
- The official fixture recipe regenerated `web/demo.awfb`; the Web/native
  parity suite then passed all seven tests.

## Design corrections kept out of production

### Parser error representation

The package names a repository-wide `ParseErrorKind`, but production owns a
coded `ParseError`. Environment errors use the required
`syntax.parse.style_environment.*` codes and exact ranges through that owner.
No feature-local enum or dual representation was added. The independent design
request is
[`seq-06.11d.4.2.2`](../reviews/requests/2026-07-16-seq-06.11d.4.2.2-typed-parser-error-kind-reconciliation.md).

### Product-source component containment

The current complete-product boundary can validate source identity, extent,
UTF-8 boundaries, condition/clause containment, and rule provenance. It cannot
invent individual component sources that are absent from
`ViewEnvironmentClause`, and an exact parenthesized predicate range cannot also
contain a guarded rule authored after that predicate. The implementation does
not reparse source text or manufacture offsets to satisfy those contradictory
requirements. The predecessor-aware correction request is
[`seq-06.11d.4.2.3`](../reviews/requests/2026-07-16-seq-06.11d.4.2.3-environment-product-source-contract-correction.md).

These two requests are design work, not unimplemented code from an otherwise
specified cut. They must not redesign the checked environment substrate unless
current implementation evidence demonstrates a concrete flaw.

## Structural audit

Measured on Jujutsu change `pysvkrvuzyxp` rebased onto `0865651127dd`; byte and
physical-LOC counts are from the integrated checkout, not diff additions.

| Path | Owner / kind | Bytes | LOC | Responsibilities and disposition |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-runtime-driver/tests/session.rs` | runtime-driver integration test | 106,521 | 2,945 | session lifecycle matrix; warning-level test hotspot, below the 8,000 LOC error threshold |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | CLI embedded unit tests | 81,672 | 2,547 | bundle/profile/fixture behavior; warning-level test owner |
| `crates/arcweft-view/tests/logical_axis_provider.rs` | View integration test | 61,663 | 2,112 | logical-axis/provider/cache regression matrix |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | runtime-driver integration test | 57,836 | 1,605 | typed View runtime behavior and canonical product fixtures |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | bundle product model | 58,072 | 1,771 | product model facade and checked decode; input DTOs split to `model/input.rs` |
| `crates/arcweft-bundle/src/resource_codec/view/codec.rs` | bundle codec | 56,252 | 1,559 | codec dispatch/facade; style, environment, text, and theme codecs split by responsibility |
| `crates/arcweft-lang-syntax/src/parser/style.rs` | syntax production | 50,206 | 1,388 | cohesive Style grammar/recovery owner; warning-level, no unrelated semantic work |
| `crates/arcweft-cli/src/app/runtime/run.rs` | CLI production with embedded tests | 48,627 | 1,335 | runtime launch/watch orchestration; Profile/Project boundary corrected here |
| `crates/arcweft-bundle/src/product.rs` | bundle production with embedded tests | 44,048 | 1,207 | complete-product construction and validation; warning-level boundary owner |
| `crates/arcweft-runtime-driver/src/session.rs` | runtime-driver facade | 44,496 | 1,186 | public session API; construction, environment, hot-swap, lifecycle, persistence, and text control split into modules |
| `crates/arcweft-player-native/src/scene_windowed.rs` | native facade with embedded tests | 38,582 | 1,067 | window/event-loop facade; frame/input orchestration split into modules |
| `crates/arcweft-tooling/src/style_environment.rs` | tooling production | 34,376 | 950 | one cohesive transport-neutral environment feature owner |

The canonical audit command reported 3,026 files, 1,501 Rust files, 693,681
Rust LOC, 90 manifests, zero errors, and 128 warnings. Warnings are existing or
documented review triggers; no error-level size/dependency exception was added.

## Verification evidence

| Validation | Result |
| --- | --- |
| Presentation environment tests | 12 passed |
| Syntax / HIR / sema environment suites | 8 / 2 / 8 passed |
| Compiler environment tests | 1 passed |
| View logical-axis / environment / style-sheet suites | 12 / 15 / 14 passed |
| Bundle resource codec / environment codec / style program suites | 29 / 5 / 5 passed |
| Runtime environment unit tests | 4 passed |
| Native environment / scene prepared-stamp tests | 7 / 1 passed |
| Tooling / LSP environment suites | 14 / 1 passed |
| Real Chrome browser environment harness | 30 checks passed |
| Web parity suite after fixture refresh | 7 passed |
| Web wasm32 all-features check | passed |
| `just test-fast` | 324 tests passed across its five crate groups |
| `cargo check --workspace --all-targets --all-features` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed on the integrated checkout |
| Post-rebase syntax / HIR / sema / tooling / LSP suites | 8 / 2 / 8 / 14 / 1 passed |
| `cargo +nightly -Zscript tools/structure-audit.rs --root .` | passed with zero errors and 128 warnings |

The first `just test-workspace` attempt stopped only because the former target
layout filled the disk. After cleanup, the next run passed the workspace except
for two `arcweft-player-web` parity assertions against the stale checked-in
`web/demo.awfb`. `just fixture-refresh-web-demo-awfb` regenerated that artifact
successfully, and the complete parity test binary then passed 7/7.

The final cold-target workspace runs exposed three stale runtime View fixtures;
after the canonical inventory repair, their complete focused binary passed
15/15. The following workspace run passed that runtime stage and all
environment-related stages, then failed one unchanged CLI unit test:
`stdio_transport_times_out_and_retains_bounded_stderr_tail`. Under concurrent
cold builds, its PowerShell child did not start and write `tail-end` within the
test's fixed two-second startup budget; an exact rerun reproduced the same
host-load failure. No environment production code participates in that test.
This is recorded as composite evidence rather than misreported as a one-shot
workspace pass.

The bundled Playwright Chromium could not start locally because its Windows
distribution lacked `dxil.dll`; the same generated wasm harness passed all 30
checks with the installed Chrome channel. Tier-2 MCP/resource and exact visual
golden suites were not required by this Style-environment cut. Doc-tests are a
separate policy lane and were not included in the recorded workspace run.

After preserving the tracked fixtures, implementation note, and task-level
verification results outside build output, the recreated dedicated
31,496,087,121-byte `target` directory was removed. No comparison artifact
required for integration was stored only under `target`.

## Remaining work

There is no known implementation-ready environment-condition cut remaining.
Only the two independently throwable design corrections above remain.
Formatting, workspace check/Clippy, the normal workspace fast path, wasm target
check, focused environment suites, the real Chrome harness, and the structural
audit were rerun from the integrated checkout.
