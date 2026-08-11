# Repository evidence

## Baseline

```text
GIT_COMMIT=0c8cb74dd96116a8b987cc419c9a280b6cabe4a4
INSPECTION_DATE=2026-08-08
```

Current source, maintained docs, Cargo manifests, and tests at this commit were
used as implementation authority.

## Repository policy

`AGENTS.md` establishes:

- latest accepted `main` as the working baseline;
- current source and maintained contracts as authority;
- preservation of layer direction and Sans-I/O ownership;
- one final typed authority, no dual readers or copied side tables;
- deletion-driven migration;
- adding behavior to Arcweft-owned types instead of helper/extension-trait
  detours;
- typed/executable/codec/dependency evidence instead of source gates;
- Git-only version-control evidence; and
- explicit passed/failed/blocked/not-run validation reporting.

The applicable Rust skill was read completely. This package introduces no Rust
code, but its API/design choices follow the newtype/serde/public-boundary and
Clippy/fmt expectations.

## Historical request and intake evidence

- `docs/reviews/requests/2026-07-20-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation.md`
  — resolved on 2026-07-21; do not dispatch; records the corrected package hash
  and the fixed owner/admission/source-map/wire decisions.
- `docs/implementation/2026-07-20-lang-01-5-1-1-dialogue-profile-presentation-owner-intake.md`
  — selects the corrected Lang-01.5.1.1 package, preserves launch ownership,
  chooses compiler admission, and corrects the reusable revision's physical
  owner to a lower cycle-free boundary.
- `docs/implementation/2026-07-22-lang-01-5-1-1-checked-dialogue-profile-admission.md`
  — records completion of checked admission, runtime authority, deletion of
  source defaults/orphan selectors, consumer migration, and validation.

## Current source evidence

### Launch owner

`crates/arcweft-launch/src/manifest.rs`

- `ArcweftManifestDocument` and `ProfileSpec` are strict kebab-case records.
- `DialogueProfileSpec` is crate-private and owns optional typed `view`,
  `style`, and `inline_failure` fields.
- `arcweft-launch` imports the dialogue policy and View/Style IDs; the neutral
  manifest model does not own them.

`crates/arcweft-launch/src/accepted.rs`

- `SourceBackedManifest` contains one document, one typed manifest, and one
  generic map.
- `decode` parses once and verifies document `Arc` and identity coherence.
- `manifest_token_span` is the public typed projection API.
- `resolve_profile` performs no I/O or reparse.

`crates/arcweft-launch/src/source_map.rs`

- one `ManifestTokenPath` family covers dialogue table, View, Style, inline
  policy, fallback, style policy, and style elements;
- slots are table header, field key, and value;
- map construction validates every span against one document.

`crates/arcweft-launch/src/resolve.rs`

- resolution constructs `DialoguePresentationProfile` field-wise;
- omitted View uses `ViewId::standard_dialogue()`;
- omitted inline policy uses `Default` (`FailLine`).

`crates/arcweft-launch/src/diagnostic.rs`

- closed manifest codes include `UnknownField`, `IdInvalid`, `IdFamily`, and
  `InlinePolicyInvalid` with stable strings.

### Dialogue owner

`crates/arcweft-dialogue/src/presentation_profile.rs`

- immutable typed profile;
- engine default is standard dialogue View, no style, fail-line policy.

`crates/arcweft-dialogue/src/inline_failure.rs`

- strict tagged policy/fallback/style enums;
- unknown fields rejected at every level.

`crates/arcweft-dialogue/src/presentation_revision.rs`

- exact six typed facts;
- strict serde and structural equality;
- codec tests reject missing, unknown, malformed, and noncanonical values.

### Compiler admission owner

`crates/arcweft-compiler/src/project/dialogue_profile.rs`

- `CheckedDialogueProfile` retains owner, presentation, revision, same accepted
  product Arc, and selected View/Style source provenance;
- `try_admit` is the single Sans-I/O construction operation;
- launch admission re-resolves the same manifest and requires registry Arc
  identity plus digest coherence;
- product admission checks program/style source revisions, existence, canonical
  dialogue role, and provenance;
- error-to-diagnostic mapping uses the four stable profile codes and one
  definition secondary label.

`crates/arcweft-compiler/tests/dialogue_profile_admission.rs`

Directly observed tests cover:

- same product Arc and all revision fields;
- omitted fields/standard View;
- project-default owner;
- missing View, non-dialogue View, missing Style codes and stage; and
- exact resource registry Arc rejection.

### Cargo ownership evidence

Current Cargo manifests preserve the neutral lower manifest model, lower
shared dialogue revision, compiler admission, and no project-loader to
runtime-driver reversal. The final structured acceptance test must use
`cargo metadata`, as specified in `TEST_MATRIX.md`.

## Evidence limitations

This return inspected the current remote snapshot and locally retained raw
source excerpts, but did not have a complete repository checkout. It therefore
did not execute Cargo, Clippy, the workspace tests, Tier 2, or parity suites.
The broader green results are cited only as repository-recorded implementation
evidence, not as newly executed validation.
