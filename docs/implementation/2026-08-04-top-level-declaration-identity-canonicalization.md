# Top-level declaration identity canonicalization

Date: 2026-08-04

## Baseline

- Git commit: `fde3bd0af47ff32c279a9c26d8692cbf12b8d1ff`
- Branch: `main`
- Working tree: dirty before this task; many pre-existing changes are present.
- Staged changes: none at task start.
- Planning: Sol (`gpt-5.6-sol`, max) produced the implementation plan before
  source changes began.

This note is a task ledger, not a source-spelling acceptance gate. Each
declaration is reviewed in context; lint/LSP cases, relative-ID language
examples, generated round trips, entry/test/bench contracts, and historical
implementation/review records are not canonicalization targets unless a later
design decision says otherwise.

## Selected contract

- The canonical authored form is a local declaration name without `@`.
- An explicit declaration identity with a local name remains accepted for
  generated or elaborated surfaces and receives the shared identity-lint
  policy.
- `proof @proof:.name name(...)` remains accepted and is normalized through the
  same typed identity path as other top-level declarations.
- Bare, absolute, family-relative, and short-relative spellings are accepted
  by the shared declaration producers and normalized to one family-qualified
  public ID before attachment.
- A redundant explicit identity reports AWF0101 unless `generated` or the
  matching style allowance suppresses it.
- Binding mismatch and wrong-family identity are diagnosed without discarding
  the declaration owner or inventing a replacement ID.
- Entry/test/bench helper behavior is outside this cut.
- The public parser/HIR/sema/verifier authority switch, detached proof clause
  reader deletion, and full stringly-identity migration are separate cuts.

## Work ledger

| Scope | File/range | Family | Classification | Current form | Proposed result | Owner | Reviewed | Validation |
|---|---|---|---|---|---|---|---|---|
| Syntax domain | `arcweft-id`, declaration projection | all retained/callable declarations | `CANONICALIZE` | family and identity rules are split | shared typed identity parser/projection introduced; declaration-header projection is shared by retained and callable declarations, while retained HIR storage remains only at its legitimate boundary | primary | reviewed | `cargo check` passed |
| Modern syntax | proof parser/attachment/HIR | proof | `CANONICALIZE` | explicit ID is raw syntax and is dropped | explicit, family-relative, and short-relative IDs reach attachment and final HIR; wrong-family recovery is retained | primary | reviewed | proof parser/attachment/HIR tests passed |
| Legacy syntax | headers/proof/source | proof/source | `CANONICALIZE` | proof-only identity-and-name helper, absolute-family gap, and source full-ID rejection | shared required/optional-name identity parser; source may retain an explicit ID without inventing a duplicate local name; known absolute family mismatch is diagnosed without dropping the item | primary | reviewed | syntax proof/source tests passed; sema relative declaration test passed |
| Modern identity validation | shadow flow/source/style grammars and attachment | flow/source/style | `CANONICALIZE` | each grammar re-derived family membership from local string/root matches | typed `SyntaxIdRefSyntax::normalized_for_family` and `DeclarationIdentityFamily` provide the family authority; source marker metadata comes from the lexer projection | primary | reviewed | syntax check and 78 source-focused tests passed |
| Syntax lint | proof identity lint | proof | `CANONICALIZE` | raw source `@` check and missing attrs | parser-owned explicit origin and attrs drive the existing lint policy | primary | reviewed | lint proof/style tests passed |
| Tests | syntax/sema/compiler/LSP/fixtures | all | `LINT_LSP_INTENTIONAL` / `RECOVERY_NEGATIVE` / `CANONICALIZE` | mixed by intent | reviewed case by case; Wave 1 covered syntax, sema, and compiler tests; Wave 2 covered CLI/tooling/runtime-plan/verify/fixtures | Luna Wave 1 and Wave 2 owners | reviewed for Waves 1–2 | Wave 1: 59 replacements and 81 intentional residual cases; Wave 2: 215 replacements and 73 intentional residual cases; no unresolved case decisions |
| Stable docs | active language/tooling/examples | all | `CANONICALIZE` / `RELATIVE_ID_CONTRACT` | mixed authored and explanatory forms | canonical authored examples; explicit policy documented | docs owner / Luna Wave 2 | reviewed for active docs in Waves 1–2 | Wave 2 docs: 30 canonicalizations, 48 intentional residual cases, 0 unresolved; aggregate canonical test timed out |
| CLI/tooling/runtime verification | CLI checks, runtime plan, tooling, verify, verify-LSP | character/source/signal/flow | `CANONICALIZE` / `LINT_LSP_INTENTIONAL` / `RECOVERY_NEGATIVE` | mixed test and generated surfaces | canonical authored candidates; typed signal identities and mismatch/recovery cases retained | Luna Wave 2 Hypatia | reviewed | 210 canonicalizations, 7 intentional residual cases, 0 unresolved; focused runtime/tooling/verify suites passed; broad CLI check remains dirty-tree-sensitive |
| Fixtures and player surfaces | `.arcw` fixtures, player-native/web | all | `CANONICALIZE` / `RELATIVE_ID_CONTRACT` / `RECOVERY_NEGATIVE` | mixed authored and negative fixtures | canonical authored candidates; relative IDs, ID-only images, and recovery examples retained | Luna Wave 2 Darwin | reviewed | 5 canonicalizations, 18 intentional residual cases, 0 unresolved; native checks passed; web parity timed out |
| Final active-doc audit | active presentation/runtime/examples docs | image/simple layer/audio/capture | `CANONICALIZE` | fully-qualified authored declaration examples remained after Wave 2 | ordinary declaration names retain the same family suffix without the explicit `@` identity; hierarchical layer namespaces remain explicit when a bare name would change the ID or violate the retained grammar | primary | reviewed | manual context review; `git diff --check` passed |
| Historical docs | `docs/implementation`, `docs/reviews` | all | `HISTORICAL_EXCLUDED` | historical evidence | unchanged | none | n/a | not run |
| Entry/test/bench | helper and invocation contracts | entry/test/bench | `ENTRY_TEST_BENCH` | existing helper contract | unchanged | none | n/a | not run |
| P1/P2 cleanup | declaration family and header projection names | all declaration producers | `CANONICALIZE` | compatibility aliases and retained-only projection names obscured the shared authority | aliases removed; retained relative IDs normalize through the same typed path; proof uses the shared declaration-header projection; cross-layer acceptance matrices cover valid and wrong-family recovery | primary / Sol max review | reviewed | selected compile, parser, lint, HIR, sema, and trusted-proof tests passed |
| Character owner provenance | `arcweft-lang-sema/src/character_definition.rs` | character owner references | `AD_HOC_READER_REMOVED` | sema stripped `@` and rejected delimiters from a raw source slice | owner inventory consumes parser-owned absolute/authored/delimited/body-range facts; symbol-path parsing remains only the typed path-to-resolution conversion | primary / Luna + Sol max review | reviewed at three-agent boundary | character-definition focused suite: 65 passed; `git diff --check` passed |
| Dialogue durable identities | `arcweft-id/src/dialogue.rs`, HIR lower IDs | say/text | `SHARED_TYPED_AUTHORITY` | HIR duplicated line/text family parsing and text-key derivation | `arcweft-id::dialogue::{DialogueLineId, DialogueTextKey}` owns family prefixes, validation, and typed line-to-text derivation; HIR keeps only speaker-surface parsing pending the larger carrier migration | primary / Luna + Sol max review | reviewed at three-agent boundary | arcweft-id dialogue: 7 passed; HIR dialogue identity: 2 passed; selected crate check passed |
| Flow lowerer family authority | `crates/arcweft-lang-hir/src/lower_ids.rs` | flow | `SHARED_TYPED_AUTHORITY` | flow IDs were constructed/compared with repeated literal prefixes | flow construction and exact-boundary extraction use `DeclarationIdentityFamily::Flow`; choice/text/entry namespaces remain in the P3 design split | primary / Sol max review | reviewed at three-agent boundary | selected HIR check and Clippy passed; `git diff --check` passed |
| P3 public authority switch | checker helpers, dialogue callee, lower IDs, entry checker, full family map | all semantic/public-ID consumers | `DESIGN_SPLIT` | raw callee/family strings and incomplete family registry remain | deferred as one typed syntax→HIR→sema migration; independently throwable contract recorded in `docs/reviews/requests/2026-08-05-lang-01.5-typed-public-id-family-authority.md` | Sol max + Luna review | design split confirmed at three-agent boundary | no implementation credit beyond the three safe subcuts above |

## Continuation 2026-08-05

- At continuation start, Git was at `fde3bd0af47ff32c279a9c26d8692cbf12b8d1ff`
  on `main`.
- The working tree remains dirty with the pre-existing broad task changes and
  the focused changes recorded in this note. The current six-path continuation
  cut is staged explicitly; all other dirty paths remain untouched.
- Sol max reviewed the remaining P3 boundary and advised against individual
  string-check replacements. Two Luna reviews independently confirmed the
  typed character-owner change and identified the existing HIR dialogue
  identity duplication. The three-agent review boundary completed.
- The P3 design split is intentional: `entry`, dialogue callee, checker
  family aliases, remaining choice/text generated IDs, and the closed family
  registry require a shared typed carrier and final-HIR authority before old
  readers can be deleted.

## Validation record

### Performed

- Read applicable Rust, documentation, and implementation validation
  instructions.
- Reconfirmed the dirty Git state without modifying unrelated paths.
- Obtained the Sol max implementation plan.
- Completed Luna Wave 1 review: 55 files/cases across syntax, sema, and
  compiler tests; 59 case-level canonicalizations and 81 intentional residual
  cases were reported, with no unresolved case decisions.
- Completed Luna Wave 2 review at the requested three-agent boundary:
  - Fermat reviewed 21 active documentation files: 30 canonicalizations, 48
    intentional residual cases, and 0 unresolved.
  - Hypatia reviewed 217 CLI/tooling/runtime-plan/verification candidates: 210
    canonicalizations, 7 intentional residual cases, and 0 unresolved.
  - Darwin reviewed 195 fixture files: 5 canonicalizations, 18 intentional
    residual cases, and 0 unresolved.
- Audited ad hoc family checks in shadow-flow, source-grammar, style, and proof
  paths; the non-generic flow helper and raw source-header lint check were
  removed in favor of typed declaration identity provenance and family rules.
- Applied the Sol max follow-up cut: removed `RetainedIdentityFamily`,
  `AttachedRetainedPublicId`, and `PendingRetainedHeaderProjection` compatibility
  names; renamed the parser/snapshot declaration-header projection boundary so
  proof and retained declarations use the same parser-owned storage.
- Unified modern retained-header relative-ID handling. `@family.name`,
  `@family:.name`, and `@.name` now normalize to the declaration family before
  attachment and HIR; they are not rejected as a retained-only syntax error.
- Added cross-layer acceptance coverage for bare, absolute, family-relative,
  short-relative, explicit proof, generated/allowed proof lint behavior, and
  wrong-family recovery without fabricated IDs.
- Completed a final active-document audit and canonicalized remaining authored
  examples for image, simple layer, mixer/ducking/BGM, capture, and related
  surfaces. Hierarchical layer namespaces remain explicit where the retained
  grammar cannot preserve their public ID as a bare local name. References,
  relative-ID examples, entry/test/bench contracts, and intentional ID/name
  mismatch examples remain listed as residuals rather than being rewritten.
- Updated ordinary non-lint fixtures in the dialogue revision, bundle image,
  and semantic image typecheck tests to use canonical declaration names.

### Passed

- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir --quiet`
- `cargo test -p arcweft-lang-syntax --lib predicate_proof`
- focused source identity and proof lint tests
- focused final-HIR explicit and wrong-family proof identity tests
- syntax source/canonical/metric subsets, sema test subsets, and compiler
  library tests (including compiler Clippy) reported by Luna Wave 1
- Wave 2 focused suites: runtime-plan 51, tooling 25, verify 39, verify-LSP 16,
  CLI compile-only, native live-patch 8, and five native fixture compile checks
  passed.
- `git diff --check` passed for all three Wave 2 scopes.
- The post-alias-removal syntax/HIR `cargo check` passed; syntax predicate/proof
  36, lint 20, source-focused 78, trusted-proof integration 6, dialogue
  revision 1, semantic image typecheck 1, CLI bundle image 1, HIR symbol
  identity 1, and project-index 2 focused tests passed.
- The lint suite includes a regression test for relative entity identity
  provenance without source rescanning.
- Post-cleanup focused results: retained-header parser 7, retained identity
  HIR matrix 1, explicit proof HIR matrix 1, wrong-family proof recovery 1,
  redundant-proof lint 1, trusted-proof integration 6, and character parser 6
  all passed; the sema relative-ID normalization test also passed.
- The family-relative wrong-family recovery regression test passed: an authored
  `@view:.name` on a `character` declaration retains `view.name` as recovery
  evidence instead of fabricating `character.name`.
- Character inventory now uses typed `EntityRef` authored provenance and body
  ranges instead of stripping `@` from its source slice; the focused
  `character_definition` suite passed 65 tests.
- Durable dialogue line/text validation and line-to-text derivation now use
  the lower `arcweft-id::dialogue` authority. The `arcweft-id` dialogue subset
  passed 7 tests and the HIR dialogue-identity subset passed 2 tests.
- Selected `arcweft-id`, `arcweft-lang-hir`, and `arcweft-lang-sema` checking
  passed after these changes; targeted `rustfmt` on the four changed Rust files
  and `git diff --check` passed.
- After the flow-family cleanup, the extended selected check covering
  `arcweft-id`, syntax, HIR, sema, compiler, and CLI passed; the sema entry
  subset passed; selected Clippy for `arcweft-id`, HIR, and sema passed again.
- `cargo check --workspace --quiet` passed with the current dirty working tree.
- `cargo check -p arcweft-id -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-compiler -p arcweft-cli --quiet` passed after the shared projection rename.
- `cargo clippy -p arcweft-id -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-compiler -p arcweft-cli --all-targets --quiet` exited 0; existing warnings remain.

### Failed

- The first predicate/proof run exposed a projection-marker assertion for
  `ProofItem`; the setter now accepts the shared proof owner and the full
  predicate/proof subset passes.
- The first wrong-family HIR test exposed a source-index poison-state omission;
  the proof identity recovery is now included in the source-index expectation.
- The sema Wave 1 review found a stale function-like source header in the
  relative-identity test. It was migrated to the canonical typed source form,
  and the legacy parser now accepts an explicit source identity without a
  duplicated local name.
- The first P1/P2 cleanup attempt exposed duplicate imports and an obsolete
  relative-ID recovery variant; both were removed before the selected compile
  and focused tests were rerun.
- Extending the acceptance matrix exposed one recovery reconstruction bug for
  family-relative wrong-family IDs. The recovery path now uses the authored
  family root, and the attachment/HIR checks were rerun successfully.
- The broad CLI check reported 96 passed / 309 failed / 41 ignored. Its
  representative failure (`AWBC is missing a required public entrypoint`) is
  coupled to pre-existing dirty flow canonicalization changes; the agent was
  instructed to stop before further attribution, so this is not treated as a
  pass.

### Blocked

- No code-path blocker remains for the selected canonicalization cut; the
  broad CLI failure still needs an isolated dirty-tree reproduction before it
  can be attributed to this cut.

### Not run

- Full syntax/HIR crate suites, formatting, structural audit, and full workspace
  validation remain pending. The aggregate documentation canonical test and
  player-web parity validation timed out. Compiler Clippy passed in the Wave 1
  scope; whole workspace Clippy was not run.
- `cargo test --workspace --no-fail-fast --quiet` was attempted with a 900
  second limit and timed out without a result; it is not counted as a failure
  or a pass. The leftover Cargo/rustc processes were later observed to have
  exited, but no test exit status was recoverable. The workspace check did
  pass separately.
- `cargo fmt --all -- --check` was attempted and failed on formatting differences
  across the already-dirty worktree (including pre-existing unrelated files);
  no broad formatter rewrite was applied to preserve user changes.

## Non-goals and decision points

- Do not use a regex or raw occurrence count as an acceptance criterion.
- Do not rewrite ambiguous mismatch examples until the intended public ID or
  local name is established from the surrounding test/document contract.
- Do not broaden this cut into the planned public authority switch.
- Defer the remaining P3 authority migration as one cut: checker
  `@character` string classification, dialogue callee parsing, choice/text
  generated-ID prefix handling, entry-target family classification (including
  the sema flow target), and the complete keyword/family registry. The
  independently throwable contract is
  `docs/reviews/requests/2026-08-05-lang-01.5-typed-public-id-family-authority.md`.

## Isolated completion validation 2026-08-06

The declaration cut was reassembled in the clean
`codex/proof-public-switch` worktree so that the protected integration WIP did
not participate in its acceptance result.

### Passed

- `cargo fmt --all -- --check`
- `CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets --all-features`
- `CARGO_BUILD_JOBS=1 cargo clippy --workspace --all-targets --all-features`
  (existing warnings only)
- `CARGO_BUILD_JOBS=1 cargo test --workspace --lib --tests --exclude arcweft-cli --quiet`
- `cargo test -p arcweft-cli --lib --bins --quiet`
- the CLI `runtime_native_options`, `check_core_cli`,
  `native_style_parity_sample`, `release_trust_json`,
  `responsive_stage_placement`, and
  `seq04_8_4_persistent_cache_build_cli_goldens` integration gates
- runtime-plan integration: 51 passed
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 4,117 files,
  2,244 Rust files, 1,102,502 Rust physical LOC, 0 errors, 176 warnings
- `git diff --check`

### Failed with an existing Proof authority gap

`cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet` reports two
fixture-loop failures. The first check fixture and first run fixture both
terminate with `sema.nominal.unknown_type` for capability-owned `FsError`.
Restoring the redundant explicit Flow identity does not change the result, so
this is not caused by canonical declaration spelling.

The old compiler still consumes `parsed.typed_tree()` through
`lower_document_to_hir`. That legacy `ExternCapabilityItem` retains function
members and a raw body but drops associated-type members, while the final
attached HIR already owns `HirCapabilityMember::AssociatedType` in the
capability item scope. This gap was also recorded in the 2026-07-26 and
2026-07-27 Proof implementation notes. It must close through the deletion-driven
Proof public authority switch: publish the final capability member arena, move
compiler and sema consumers to it, and delete the legacy raw-body reader in the
same compiling cut. This declaration cut deliberately does not add a global
`FsError`, raw-body reparse, fallback resolver, fixture alias, or compatibility
projection.

The aggregate `just test-workspace` therefore exits non-zero at that known CLI
gate. An earlier parallel attempt also hit Windows OS error 1455 (page-file
exhaustion); all conclusive reruns above used one Cargo build job.

## Ad hoc audit

- `crates/arcweft-lang-syntax/src/attachment/flow.rs` no longer has a
  flow-specific string/root matcher; flow family validation now uses the typed
  `DeclarationIdentityFamily` normalization path shared by source and style
  grammar boundaries.
- The lint layer no longer decides authored-vs-derived identity by scanning a
  source slice for a leading `@`; it consumes parser-owned identity provenance.
- Remaining direct `@` checks in syntax are grammar recognition for references,
  relative-ID handling, or recovery boundaries. The sema character owner
  inventory and dialogue/entry lowering checks are consumer-specific semantic
  policies, not declaration-surface lint paths. Their broader string-map/public
  authority migration is recorded as the separate P3 cut from the Sol plan and
  was not silently expanded here.
- The parser and snapshot no longer expose a retained-only header projection
  name: `declaration_header_projection` is the single storage boundary shared
  by retained declarations and callable proof declarations. `AttachedRetainedHeader`
  remains intentionally retained-only because it is the HIR-facing model for
  character/view/action/activity/signal/metric/layer item headers.
- HIR no longer owns a duplicate `say`/`text` identity family parser or
  line-to-text string derivation; those operations are on the typed
  `arcweft-id::dialogue` domain. Raw speaker callee normalization remains a
  known P3 boundary until syntax/HIR carries the typed callee facts.
- HIR flow ID construction and exact family extraction now use
  `DeclarationIdentityFamily::Flow`; choice/text generated paths and entry
  checks remain intentionally in the P3 split because their owning family
  registry is not yet complete.
