# AW-AH-009.4 Cut 1 — CharacterDialogue runtime/domain substrate

Date: 2026-07-19

## Source and baseline

This cut implements only Cut 1 from
`arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract.zip`.
The package was inspected before implementation, reports no open questions, and
has SHA-256
`a86044fea7aaff3ec3829dfa0ad6552c88377ca61fa2911c3b96ea34ca0ffa5e`.

The implementation started from `main` commit
`7361c21e7488` (`Unify dialogue View observation and Tier 2 contracts`).

## Implemented contract

### Generic nominal runtime value

- `RuntimeNominalRecordValue` retains a checked nominal type identity, exact
  layout hash, and schema-ordinal field values separately from anonymous
  `RuntimeValue::Record`.
- `RuntimeValue::NominalRecord` participates in deterministic canonical bytes,
  digesting, labels, nested runtime validation, replay-safe traversal, runtime
  summaries, and every current exhaustive consumer.
- `RuntimeValue::validate_nesting_depth` and
  `MAX_RUNTIME_VALUE_NESTING_DEPTH` own the inherited 64-level runtime-value
  rule. Dialogue structured field paths do not redefine this generic runtime
  boundary.
- Canonical floating-point bytes reject non-finite values and normalize
  negative zero to positive zero.
- The owning `RuntimeValue` API recognizes only pathless or explicitly
  `Option`-qualified, well-formed `Some`/`None` values when constructing the
  corresponding empty option. A same-named variant from another nominal path
  is not treated as `Option`.
- Runtime `Option` recognition, construction, and intrinsic evaluation now
  live in the cohesive `value/option_value.rs` responsibility module. This
  keeps the main value owner below the structural-audit error threshold while
  preserving its public API.
- The added `RuntimeValue` variant increased the size of `RuntimeExpr` enough
  to trip the existing control-stack enum's large-variant lint. The
  `WhileLet` control-stack frame now boxes its optional guard while the
  executable `FlowOp` contract remains unchanged.

AWBC constant lowering and generic data conversion fail closed rather than
projecting a nominal value to an anonymous record. Nominal AWBC
ABI/codec/type-table support is owned by AW-AH-009.4 Cut 4.

### CharacterDialogue domain

- `CharacterDialogue` directly owns `CharacterId`,
  `CharacterDialogueContractIdentity`, exact layout identity, and one immutable
  validated `CharacterDialogueConfig`.
- Role-specific Rust newtypes cover voice, stage, portrait, focus, cleanup,
  hook, style, rich-text style, and custom values. Public fields remain private,
  and every role's deserialization routes through its role-specific
  size/shape-validating constructor.
- `CharacterDialogueLimits` has the frozen 16 public fields with their exact
  `u8`/`u16`/`u32` widths.
  `PRODUCTION_CHARACTER_DIALOGUE_LIMITS` is the single production policy value;
  the removed unit struct and associated `usize` constants are not retained.
- Generic typed values use the inherited runtime nesting limit of 64, reject
  noncanonical anonymous-record order, reject non-finite floats, and store
  recursively normalized positive zero. The depth-8/256-leaf limits apply only
  to style/rich-text structured values and schema-ordinal field paths.
- Stage, portrait, focus, cleanup, and custom values enforce the 64 KiB field
  cap. Style and rich-text values enforce depth 8, 256 total leaves, and
  256 KiB aggregate bytes. Hook lists enforce 64 entries and 256 KiB aggregate
  bytes without incorrectly limiting arbitrary inner sequences or function
  captures.
- `DialogueLocaleId` validates and canonicalizes the bounded ASCII BCP-47
  subset. Voice, View, look, custom-field, line, and text-key identities
  validate both their existing nominal/family rules and the exact field-table
  byte limits. `CharacterId` retains only its owning crate's validation because
  the package assigns it no CharacterDialogue-specific byte limit.
- `CharacterDialoguePatch` applies to a cloned candidate, validates the
  complete result, and returns only an accepted immutable value. Failure leaves
  the base value unchanged.
- Optional fields, the required standard View, hook lists, inline-failure
  policy, structured style/rich-text values, and custom keys use their frozen
  clear semantics.
- Structured nominal clear preserves nominal type, layout, and field count. It
  converts only a provably option-like or structurally empty field to its empty
  representation and rejects an unprovable clear atomically. Anonymous records
  retain their independent sparse-field removal behavior.
- `CharacterDialogueRuntimeSchema` is the Cut 1 context owner for canonical
  18-field carrier encode/decode and validates the accepted Character catalog,
  character manifest digest, look ownership, accepted View registry, custom
  schema digest, custom declared type/layout, accepted Views, sorted custom
  entries, and canonical runtime representation. Decode always returns the
  re-encoded canonical record, so its paired record and domain value cannot
  diverge after negative-zero normalization.

The inline-failure policy types moved from `arcweft-render-text` to
`arcweft-dialogue` with no render-text re-export. `FallbackStylePolicy::Apply`
now carries checked `CharacterDialogueStyleValue` entries rather than the
renderer-owned `RichTextStyle`; retaining the renderer type would create the
forbidden `dialogue -> render-text -> dialogue` dependency cycle. Existing
render-text/runtime-plan consumers now import the policy from its dialogue
owner.

### Dependency-contract reconciliation

The package states that `arcweft-dialogue` may depend on `arcweft-core` because
none of the listed lower-layer crates depends on `arcweft-dialogue`. The current
checkout contained one contrary production edge:
`arcweft-lang-syntax -> arcweft-dialogue`, used only to share renderer-neutral
rich-text tag and built-in effect vocabulary. Adding the package-required
`dialogue -> core` edge therefore made
`arcweft-lang-sema -> syntax -> dialogue -> core` reachable and failed the
workspace dependency-direction test.

The shared rich-text vocabulary now belongs to the existing
`arcweft-presentation::rich_text` responsibility module. Sema, tooling, LSP,
render-text, and runtime-plan consume that lower renderer-independent owner
directly. Syntax uses it only in dev tests, has no production dependency on
presentation or dialogue, and does not pass the vocabulary through its public
API. Dialogue likewise does not re-export the old module path. This is a direct
ownership correction rather than a compatibility layer. It preserves both the
package-required `dialogue -> core` runtime-value edge and the repository's
protected language-layer dependency boundary.

### Direct deletion

The Rust dialogue crate no longer defines or re-exports:

- `SpeakerRef`;
- `SpeakerPreset`;
- `SayOptions`;
- `VoicePolicy`;
- `DialogueLineBuilder`;
- the `.say()` builder API.

The compile-fail API examples prove the removed public names and `.say()` method
do not resolve.
There is no alias, deprecated wrapper, dual representation, removed-spelling
diagnostic, or source gate.

## Deliberately retained later-cut substrate

This cut does not change source syntax, HIR, sema, executable dialogue
lowering, display products, session persistence, or Agent presentation
observation.

In particular:

- syntax `.say` fixtures remain until the source/HIR/sema switch in Cuts 2 and
  3;
- `arcweft-runtime-plan::DialogueSpeakerPreset`,
  `speaker_preset_from_let`, `speaker_preset_chain`, and their `.say` fixtures
  remain until their explicitly assigned atomic deletion in Cut 4;
- AWBC remains on the current ABI/codec and rejects nominal constants instead
  of erasing their identity;
- display, bundle save/replay, hot reload, and active Agent dialogue
  projection remain assigned to Cut 5;
- TTS/voice-catalog work from AW-AH-009.4.1 is not part of this package cut.

The following validation is intentionally owned by later accepted-program
boundaries rather than inferred from an effective Cut 1 value:

- `CharacterDialogueRuntimeCustomFieldDescriptor::clearable` is consumed by
  Cut 3 sema for `SEM-028`/`AW-CD-016` and by the Cut 4 typed patch
  plan/AWBC verifier. After a valid `Clear`, the effective config intentionally
  stores no key, so `validate_dialogue` cannot and must not distinguish a
  tombstone from an initially absent key.
- defaults and aggregate View-contract digests remain value identity in Cut 1.
  Cut 4 accepted AWBC tables provide expected digests, and Cut 5
  runtime-driver/hot-reload compares them for `PER-018`/`PER-019` and
  `AW-CD-R008`/`AW-CD-R009`. The frozen Cut 1 runtime-schema context has no
  expected defaults/View digest input.
- the package names fixed source/runtime roles such as `DialogueStage`,
  `DialogueHook`, and `RichTextStyle`, but does not publish their exact
  `RuntimeNominalTypeId` spellings or layout hashes and does not put a role type
  table in the frozen Cut 1 runtime-schema context. Cut 1 newtypes therefore
  prevent direct Rust API interchange and validate generic nominal
  identity/layout correlation, size, depth, and canonical form; Cut 3 resolves
  source expected types and Cut 4's accepted AWBC type table/verifier enforces
  each exact role nominal/layout. Until that atomic switch,
  `CharacterDialogueRuntimeSchema` remains a provisional domain carrier and can
  encode a structural style used by domain patch tests; it is not claimed as
  completed exact AWBC wire-role validation.
- patch work 1,024/1,025, outer sequences containing 4,096/4,097
  `CharacterDialogue` values, direct function captures containing 256/257 such
  values, FX application count, and defaults-table count are Cut 4
  verifier/VM/table responsibilities. Cut 1 therefore exposes their exact
  production-limit fields but does not misapply them to arbitrary values
  nested inside one dialogue config.

These are sequenced implementation boundaries from the package, not
compatibility promises.

## Direct tests

The Cut 1 tests cover:

- CharacterId ownership and immutable patching;
- required View, optional, hook, inline-failure, and custom clear behavior;
- successful and failed patch transactionality;
- nominal field-count-preserving leaf clear and `clear_all`;
- anonymous record sparse-field clear as a distinct behavior;
- fail-closed non-Option `.Some` clear with base digest unchanged;
- nominal versus anonymous canonical identity;
- canonical equality/digest and negative-zero normalization;
- decode record/domain canonical agreement after negative-zero normalization;
- stored nested negative-zero normalization and strict anonymous-record order;
- non-finite value rejection;
- inherited runtime nesting at exact depth 64 and one-over 65;
- structured path/value depth 8/9 and total leaves 256/257;
- exact role encoded-size boundaries and one-over rejection;
- exact 256 KiB hook aggregate and one-over rejection;
- validated role deserialization rejecting an over-deep structured value;
- exact/one-over voice, View, look, line, and text-key ID limits;
- exact outer nominal type, layout, and 18-field count rejection;
- reverse custom-entry order rejection;
- exact runtime-schema round trip;
- direct compile-fail evidence for all deleted Rust dialogue APIs, including
  `VoicePolicy`, `DialogueLineBuilder`, and the `CharacterDialogue::say`
  method.
- direct Cargo-metadata dependency evidence that `arcweft-lang-sema` does not
  reach `arcweft-core`, while the package-required dialogue/core edge remains
  acyclic.

## Validation

Completed:

- `cargo fmt --all -- --check`;
- `cargo check -p arcweft-core -p arcweft-dialogue -p
  arcweft-render-text --all-targets --all-features`;
- `cargo test -p arcweft-core -p arcweft-dialogue -p
  arcweft-render-text --all-features`
  - `arcweft-core`: 209 unit tests and 9 runtime-ID integration tests;
  - `arcweft-dialogue`: 22 unit tests;
  - `arcweft-render-text`: 9 unit tests, 11 frame-resolution tests, and 12
    resolved-document tests;
  - dialogue compile-fail doc tests: 4;
  - all passed;
- `cargo clippy -p arcweft-core -p arcweft-dialogue -p
  arcweft-render-text --all-targets --all-features -- -D warnings`;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- the normal `just test-workspace` command set:
  - non-CLI workspace lib/integration tests passed;
  - CLI lib/bin tests: 207 passed;
  - the seven selected CLI integration binaries: 22 passed;
- `just test-doc`, including the four dialogue API compile-fail examples;
- `cargo test -p arcweft-project-loader --test dependency_direction`: 4
  passed;
- `just test-tier2`
  - MCP stdio and Agent observation integration: 23 passed;
  - native capture, animation, mask/object-ID, typewriter/ruby, visual-smoke,
    checked-in IMQ golden, and exact CLI check slices: all passed.

One pre-final non-CLI workspace run observed the existing
`release_remote_publish_file_mirror_archive_verifies_after_publication` test
fail once because its Windows temporary staging path disappeared during
publication. The exact test passed immediately afterward, and two subsequent
non-CLI workspace runs passed, including the final ownership graph. No
production change or test relaxation was made for that transient failure.

The canonical structural-audit output for this cut is stored under
`docs/implementation/structure-audits/aw-ah-009-4-cut1/`. It scanned 3,281
files, 1,683 Rust files, 773,847 physical Rust lines, and 92 package manifests,
and reported 0 errors and 132 warning-level ownership thresholds. New
CharacterDialogue responsibility modules are each below 1,200 physical LOC;
the largest is the 835-line context-owned runtime schema.

## Explicit non-goals

No compatibility layer, source gate, removed-spelling parser diagnostic,
source-syntax migration, HIR/sema redesign, AWBC ABI switch, display/save
integration, CSS/Takumi path, TTS selection, or voice catalog was added.
