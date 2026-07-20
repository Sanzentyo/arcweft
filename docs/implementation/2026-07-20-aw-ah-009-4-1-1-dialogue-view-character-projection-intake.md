# AW-AH-009.4.1.1 dialogue View Character projection intake

Date: 2026-07-20

## Package evidence

The authoritative package is:

```text
D:\sanze\Downloads\arcweft-aw-ah-009.4.1.1-dialogue-view-character-projection-production-contract-correction-final-contract.zip
```

Its SHA-256 is
`e8aeeb318a753d79cabcb041589c110b21c0f52a4a7d42df7e84a84cff864915`.
The archive contains 16 unique, safe entry names. All 15 manifest rows match
their recorded byte lengths and SHA-256 values. `OPEN_QUESTIONS.md` contains
only `none`, and the package reports `READY_FOR_IMPLEMENTATION`.

The repository request file is
`docs/reviews/requests/2026-07-20-aw-ah-009.4.1.1-dialogue-view-character-projection-production-contract-correction.md`
with SHA-256
`49b6cd0d2cf8e53c9d0c086de90d7d5036fd3f2f69b2e1d4f5bd043c08928386`.

The package audited baseline `27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9`.
Implementation intake started from repository commit `f8221feb1`. The final
Cut-1 validation below ran at repository revision `0fa18e252fe3`.

## Acceptance classification

The package defines four ordered cuts.

- Cut 1 is pushable, inert infrastructure: the strict shared `LocaleTag`,
  accepted Character presentation-name model, checked dialogue presentation
  plan, and the single manifest-profile localization model.
- Cut 2 is pushable only after Cut 1: the sole strict LocaleCatalog compact
  codec and typed build transaction.
- Cut 3 is one atomic executable/schema replacement. It must not be divided
  into a successful mixed flat/nested, old/new AWBC, old/new frame, or
  schema-1/schema-2 interval.
- Cut 4 is validation and structural closure.

Lang-01.5.1 published its single decoder/profile owner at commit
`de4db7b78ebf`. Cut 1 now uses that owner directly:

- `arcweft-id::LocaleTag` owns strict canonical locale validation and canonical
  serde;
- `arcweft-character::presentation_name` owns the checked values, ordered
  policy, accepted catalog, generated keys, revisions, exact digest
  transcripts, and deterministic lookup;
- `arcweft-dialogue::character_presentation` owns checked target evidence and
  the catalog-bound presentation plan;
- `arcweft-manifest-model` owns private-field
  `ProfileLocalizationSpec`/`CharacterNameLocalePolicySpec` values; and
- the one `arcweft-launch::SourceBackedManifest` decoder owns
  `[profiles.<id>.localization.character_names]`, exact element diagnostics,
  and revision-bound `ManifestTokenPath` coordinates.

No Character-name TOML reader, raw map, environment fallback, catalogue
builder, or runtime projection was added alongside those owners.

## Required predecessor state

Cut 3 remains sequencing-blocked, not design-blocked, until all of these final
owners have landed and are consumed:

- the complete public AW-AH-009.4.2 bracket/colon application CST/AST/HIR
  owner;
- AW-AH-009.4.3 source-site line identity and project diagnostics;
- Proof 01.1.1.2 typed Character declaration metadata and ranges; and
- Lang-01.5.1 single manifest decoder/profile localization ownership
  (**landed at `de4db7b78ebf`**).

Current production still contains the provisional flat speaker/callee
projection, `LineDisplaySpec`, speaker fields on `LineDisplayFrame`, old View
speaker projection variants, and bundle-session schema 1. They are removed
together in Cut 3. None is preserved through a compatibility alias, dual
reader, legacy diagnostic, or transitional executable path.

TTS/provider-speaker identity is deliberately excluded and remains owned by
AW-AH-009.4.1.2.

## Validation plan

Each pushable cut receives focused unit tests, strict Clippy for changed
crates, dependency metadata inspection, `git diff --check`, and the canonical
structure audit. The final executable cut additionally requires the workspace,
Tier 2, Native/Web/headless parity, codec-tamper, Agent/MCP, accessibility, and
capture gates listed by the package.

## Current validation evidence

Cut 1 passed its connected check:

```text
cargo check -p arcweft-id -p arcweft-manifest-model -p arcweft-character \
  -p arcweft-dialogue -p arcweft-launch --all-targets --all-features
```

The final focused all-target test run passed:

```text
arcweft-character unit: 43 passed
arcweft-character integration: 3 + 1 + 1 passed
arcweft-dialogue unit: 24 passed
arcweft-id unit: 10 passed
arcweft-launch unit: 36 passed
arcweft-manifest-model unit: 14 passed
```

The strict manifest decoder test also confirms that internally tagged unit
variants reject unknown fields instead of relying on Serde's permissive unit
variant behavior.

The following gates passed:

```text
cargo fmt --all -- --check
cargo clippy -p arcweft-id -p arcweft-character -p arcweft-dialogue \
  -p arcweft-manifest-model -p arcweft-launch \
  --all-targets --all-features -- -D warnings
git diff --check
```

`cargo metadata --format-version 1 --no-deps` confirms the intended dependency
direction: `arcweft-id` has no workspace dependency, `arcweft-manifest-model`
depends only on `arcweft-id`, and `arcweft-launch` consumes the five lower
owners without introducing an inverse edge.

The canonical structural audit reported:

```text
files scanned: 3404
Rust files: 1767
Rust physical LOC: 814145
package manifests: 93
violations: 0 errors, 131 warnings
```

The largest changed production module is
`arcweft-character/src/presentation_name/catalog.rs` at 26,089 bytes and 775
physical LOC. The changed launch integration-test module is 58,245 bytes and
1,608 physical LOC, below the 2,500-LOC integration-test warning threshold.
Every other changed production Rust file is at most 773 physical LOC. The
workspace's largest Rust file remains the generated Unicode vertical
orientation table at 357,456 bytes and 12,399 physical LOC; its header records
the generator source and prohibits hand-editing the range data.

Cut 2 now has one strict compact `LocaleCatalog` resource codec. Its focused
decode, encode, tamper, ordering, and limit tests passed 21/21; strict
all-target, all-feature Clippy passed for `arcweft-bundle`; formatting and
`git diff --check` passed; and the structural audit reported 0 errors and 131
existing warnings. The accepted product/build transaction remains deferred
until the typed Character owner from AW-AH-009.4.3 and Proof is available.

Locale parsing is also no longer duplicated between dialogue and text shaping.
`DialogueLocaleId` and `LanguageTag` are narrow domain newtypes over the shared
canonical `arcweft_core::locale::LocaleId`. The full `arcweft-dialogue` and
`arcweft-render-text` test suites passed (24 + 9 + 11 + 13 tests plus four
compile-fail doctests), as did strict all-target, all-feature Clippy for both
crates.

## Remaining package work

- Cut 1: complete; publish this coherent inert infrastructure cut before
  starting Cut 2.
- Cut 2: strict `LocaleCatalog` compact family complete; accepted build
  transaction remains gated by the typed Character owner.
- Cut 3: wait for AW-AH-009.4.2, AW-AH-009.4.3, and Proof 01.1.1.2, then land
  the executable/AWBC/frame/View/save/prepared-text replacement atomically.
- Cut 4: run the package's workspace, Tier 2, parity, tamper, metadata, and
  structural gates after the executable cut.
