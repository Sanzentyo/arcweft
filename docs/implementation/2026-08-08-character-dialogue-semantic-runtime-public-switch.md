# CharacterDialogue semantic/runtime public-switch cut

- Date: 2026-08-08
- Inspected Git base: `e46278f070f3090f64a84d2de434f32974094e2f`
- Working tree: dirty with the implementation and evidence changes described
  here; this note records pre-commit evidence
- Builds on: AW-AH-009.4, AW-AH-009.4.1/.2/.3, and the accepted
  Lang-01.5.1.1.1 dialogue-profile reconciliation
- Completion credit: coherent CharacterDialogue semantic/runtime public-switch
  slice; not full AW-AH-009.4.3 matrix closure and not Lang-01.5.1 completion

## Performed

This cut makes the typed `CharacterDialogue` value the shared authority from
final semantic analysis through compilation, runtime execution, presentation,
and persistence. It does not retain a second speaker-shaped or source-derived
success path.

- The semantic environment owns exact Character and `CharacterDialogue`
  nominal identities, callable families, patch/content application schemas,
  overload facts, result types, source coordinates, limits, and deterministic
  digests. Custom fields are checked against the accepted world registry and
  preserve typed source order.
- Final analysis publishes one typed CharacterDialogue result through returns,
  aliases, branches, generic substitution, function parameters, captures, and
  dialogue-content application. Direct `return` operands are checked against
  the declared ordinary-function result; a direct terminal return no longer
  compares the synthetic HIR `Unit` tail with that result type.
- Compiler lowering consumes the final semantic facts to construct the
  CharacterDialogue value, reconfiguration patch, content application, and
  typed line metadata. It does not re-read source text or revive `.say`, the
  removed colon helper, speaker presets, or a fallback call resolver.
- Runtime-plan/AWBC preserve the nominal CharacterDialogue value and its typed
  tuple/sequence/custom-field payloads. Bundle construction has checked,
  deterministic overflow behavior rather than truncation.
- Runtime-driver, host, and native/scene players carry the same accepted
  character catalog and dialogue presentation snapshot through construction,
  display, hot swap, session save, and replay. Save/replay and live patching
  retain accepted identity rather than reconstructing it from display text.
- Text-model, render-text, render-wgpu, and player-scene consume the projected
  Dialogue View owner. Tests use shared typed Character fixtures instead of
  local string-shaped substitutes.
- LSP completion, hover, signature help, and dialogue View metadata consume
  the semantic CharacterDialogue/application schema. No source scan or
  parallel LSP-only catalog was added.

## Passed validation

All Cargo commands used the normal shared target and four build jobs. No second
worktree, alternate target directory/profile, or concurrent Cargo build was
used.

- focused semantic tests for CharacterDialogue construction, patching,
  content application, custom fields, generic/branch/capture propagation,
  signature help, and result typing — passed;
- `cargo test -p arcweft-lang-sema --lib --jobs 4` — 184 passed, 0 failed;
- `cargo clippy -p arcweft-lang-sema --all-targets --all-features --jobs 4 --
  -D warnings` — passed after the direct-return correction;
- full player-native and player-scene test suites — passed;
- relevant runtime-plan, runtime-driver, runtime-host, render-text, and
  render-wgpu suites — passed;
- `cargo check --workspace --all-targets --all-features --jobs 4` — passed;
- `cargo clippy --workspace --all-targets --all-features --jobs 4 --
  -D warnings` — passed after the final sema-only direct-return correction;
- structural audit — 2,003 Rust files, 990,187 physical Rust LOC, 94 packages,
  182 review triggers, and 0 blocking violations;
- blocking structural gate — passed;
- `cargo fmt --all -- --check` and `git diff --check` — passed at the final
  commit boundary.

## Failed validation and classified causes

### Workspace all-test link did not complete

`cargo test --workspace --lib --tests --exclude arcweft-cli --quiet --jobs 4`
did not reach a test result. The shared `target` had grown to 281.7 GiB and D:
had 0.03 GiB free; rustc/link.exe reported `no space on device` and Windows PDB
LNK1140/LNK1318 failures. This is a resource failure, not a test assertion
failure. The explicitly requested standard `cargo clean` removed 131,548 files
and 281.7 GiB. Later validation uses the rebuilt normal incremental cache.

### Tier 2 slow MCP lane

`just test-slow-mcp` completed with 3 passed and 19 failed. The failures expose
three already-separated dependency boundaries rather than a CharacterDialogue
runtime fallback to add here:

1. Most native-observe fixtures still use removed `pub image` declarations and
   legacy Character presentation fields. Final syntax requires typed resource
   declarations, whose consumer switch belongs to Lang-01.4 and
   Lang-01.5.1.2.1. Removed syntax and legacy fields are not restored.
2. The Agent script reaches entry validation after the direct-return fix, then
   fails with `sema.entry.unbound_agent_intrinsic`: current final call facts do
   not yet bind the enclosing ordinary-function owner selected as the Agent
   controller. That owner/evidence switch belongs to Lang-01.1.1; this cut does
   not infer it from syntax or reconstruct scope ad hoc.
3. One MCP hit-test fixture retains obsolete Choice syntax. The fixture is not
   weakened and the removed syntax is not given a dedicated compatibility
   diagnostic.

### Known focused-suite dependency failures

- The LSP library suite has 205 passing tests and 8 failures around `show`.
  Their typed Presentation command/AWBC ABI is externally underdesigned by
  AW-AH-011/013 and is covered by
  `docs/reviews/requests/2026-07-14-aw-ah-011-and-013-typed-presentation-command-abi.md`.
- Compiler `view_product` has 1 passing and 6 failing tests at the typed
  resource/View boundary. Dynamic View remains valid. Static classification is
  a checked optimization certificate expressed through ordinary `#[...]`
  attribution, not a language rejection or an `@static` grammar form. The
  typed resource/content-root switch is deferred to Lang-01.4 and
  Lang-01.5.1.2.1.

## Structural review

The large diff crosses semantic, compiler, runtime, and presentation layers
because the final nominal value replaces an end-to-end boundary. The cohesive
owners remain separated by layer:

- `arcweft-lang-sema` owns accepted nominal/call/type facts and diagnostics;
- `arcweft-compiler` owns final-fact consumption and runtime-plan lowering;
- `arcweft-runtime-plan` and `arcweft-bundle` own executable/wire identity;
- `arcweft-runtime-driver` owns session, display, hot-swap, and persistence;
- player/render crates consume the typed projection;
- LSP consumes semantic products without becoming a second authority.

New semantic responsibilities are split into `callable/dialogue.rs`,
`character_dialogue.rs`, and `types/character_dialogue.rs`. Test support shared
by multiple integration tests is in crate-local `tests/support` modules. The
audit found no new blocking dependency reversal, facade, shared-state,
build-script, or production/test-coupling violation.

## External design requests and how they are thrown

An unreturned correction is not represented by guessed production code. It is
written as an independently throwable Markdown request under
`docs/reviews/requests/` and contains:

- the full inspected Git SHA and dirty/clean state;
- exact repository evidence and the conflicting accepted contracts;
- only the decisions that change the implementation result;
- parent contracts, dependency order, precedence, non-goals, affected
  producers/consumers, required matrix/tests, and deletion boundary; and
- a requirement for one independently returnable ZIP that includes the final
  contract and its row/matrix evidence.

The request is then sent by itself; its filename or a conversation summary is
not treated as authority. No implementation completion is claimed until the
returned ZIP is intake-verified under `docs/reviews/packages/` and reconciled
against current main. AW-AH-011/013 above is the active example. The attached
Lang-01.5.1.1.1, Lang-01.5.1.2.1, and Lang-01.5.1.3 ZIPs have already been
intake-verified and therefore are returned dependencies, not external design
waits; their remaining status is implementation ordering.

## Not run and non-goals

- Full `just test-tier2` was not run after `test-slow-mcp` exposed the three
  classified prerequisite boundaries. Repeating the larger lane cannot close
  those contracts.
- The complete returned AW-AH-009.4.3 100-row matrix is not claimed by this
  slice.
- No source gate, compatibility alias, dual reader, fallback resolver, shim,
  CSS/Takumi path, source-string reconstruction, or removed-syntax-only
  diagnostic was added.
- This cut does not implement unreturned Lang-01.4.2, Lang-01.5.1.2,
  Lang-01.1.1.2, or Lang-01.3.1.2.1 correction content. Those boundaries remain
  excluded until their own returned authority exists. The now-returned
  Lang-01.5.1.1.1 and Lang-01.5.1.3 packages are tracked by implementation
  dependency order instead of being mislabeled as external design waits.
- The later RichText surface direction (`#expr`, `#call(...)`, and
  `#call(...)[content]` in content mode, including ruby) remains planned after
  the typed RichText and ordinary-function prerequisites. It is not inferred
  into this CharacterDialogue runtime cut.
