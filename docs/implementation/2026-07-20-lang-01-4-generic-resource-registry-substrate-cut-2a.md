# Lang-01.4 generic resource registry substrate — Cut 2A

## Status

Complete for the generic, non-contradictory Lang-01.4 Cut 2 substrate.

This cut adds an extension-neutral semantic identity/value/descriptor model
and an atomically published immutable registry. It deliberately does not
publish built-in Image/Voice/Rig descriptors, decode an extension manifest, or
attach a placeholder registry to the accepted semantic world.

## Source and revision

- Design package:
  `arcweft-lang-01.4-typed-resource-final-contract-a8403dcb.zip`
- Package SHA-256:
  `39D991C4C7361F4DA81C366B22E2282401ABD43A21F1230448F846DD38595461`
- Package manifest verification: all 11 declared files matched.
- Implementation start revision:
  `e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139`
- Final integration revision: assigned by the main-worktree integrator after
  this isolated path set is committed.

Parallel work advanced `main` while this cut was implemented in the shared
working copy. The implementation files below remain the complete selected
path set for this cut.

## Implemented contract

### Identity

- `NominalTypeId` and exact `ResourceTypeId`;
- stable schema, field, variant, codec, family, bundle-section, runtime-handle,
  and provenance identities;
- nonzero numeric field/variant/schema/codec/section versions;
- `ResourceDeclarationIdentity` preserving distinct `EntityId`, `PublicId`,
  and `ResourceTypeId` axes.

### Typed values

- closed scalar, container, nominal-record, nominal-enum, constrained-scalar,
  asset-reference, and configured-resource-reference value types;
- finite deterministic floats with canonical zero and numeric total ordering;
- the existing `LogicalDuration`, `GainDbMilli`, `PanMilli`, and `LayoutUnit`
  owner types, plus resource-owned fixed-point ratio and length values;
- the shared `arcweft-core::locale::LocaleId` owner for canonical ASCII
  BCP-47 locale values, with 64-byte, subtag-length, primary-language,
  duplicate-subtag, and case-normalization rules;
- canonical map and stable-field record storage;
- typed nested constant validation;
- `AssetRef` values carrying the current common asset `PublicId` plus exact
  payload kind;
- `ResourceRef` values carrying exact `EntityId`, `PublicId`, and
  `ResourceTypeId`.

The returned Lang-01.4.1 correction subsequently added the closed retained
Character/View/Action/Layer/Signal/presentation-target/scroll-region category
to this same owner. Its exact scope and validation evidence are recorded in
[`2026-07-20-lang-01-4-1-retained-identity-wip.md`](2026-07-20-lang-01-4-1-retained-identity-wip.md).

### Descriptors and immutable publication

- nominal record and enum schemas with stable field/variant IDs;
- field presence/defaults, capabilities, lowering binding, codec support,
  documentation, and neutral provenance;
- atomic publication with no public mutation API;
- deterministic canonical ordering and semantic digesting;
- package/provenance, nominal body, family-group, codec, capability,
  resource-reference, nested schema, and typed-default validation;
- deterministic duplicate evidence independent of input order;
- explicit integrity verification for stored schema and registry digests.

Canonical digest encoding is manual typed binary projection. It does not hash
Rust debug output, source strings, input ordering, docs, or provenance paths.
The Lang-01.4.1 retained transcript has an explicitly fallible public encoder;
registry digesting embeds that already-validated transcript with an outer
length.

### Post-review hardening

A root review after the initial focused run found two boundary issues and
corrected them before integration:

- structural constant validation could recurse through a deeply nested
  container before the registry's 64-level publication guard ran; the public
  value validator and registry default validator now share one bounded depth
  policy and return typed nesting failures;
- the inherited locale duplicate check rejected ordinary language/region pairs
  such as `de-DE`; duplicate detection now excludes the primary-language
  position while still rejecting repeated later subtags;
- duplicate schema, codec, field, and variant candidates could select a
  different temporary invalid candidate when malformed input order changed;
  candidates are now canonically ordered before validation, duplicated
  schema/codec/type identities are excluded from the tentative registry
  altogether, and reversal tests require identical publication errors,
  including same-kind/same-version schema duplicates whose malformed local
  field shapes differ.

The focused validation was rerun after AW-AH-009.3 released the Cargo gate;
the current counts and Tier 2 result are recorded below.

## Ownership and dependency audit

The model lives in the new Sans-I/O `arcweft-resource-model` crate instead of
syntax, HIR, sema, runtime, or an adapter. This keeps syntax registry-independent
and gives later semantic, bundle, save, and Agent cuts one low-level typed
owner.

- `LocaleId` now owns locale validation and canonicalization in
  `arcweft-core`; `DialogueLocaleId` and render-text `LanguageTag` are thin
  domain newtypes over that owner instead of separate string validators;
- gain, pan, duration, and layout-unit values similarly use their existing
  owner types rather than resource-local copies;
- internal fan-out: `arcweft-core`, `arcweft-id`,
  `arcweft-interaction-model`, `arcweft-layout`,
  `arcweft-manifest-model`, `arcweft-source`;
- external fan-out: `thiserror`;
- current workspace fan-in: zero;
- no filesystem, manifest path reader, parser, HIR, sema, runtime, renderer,
  Agent, MCP, capture, or platform dependency.

Fan-in is intentionally zero in Cut 2A. Consumers must be added only with the
later accepted-world and bundle/runtime cuts; no empty or fake registry was
attached merely to create a consumer.

## Changed files

- workspace membership/dependency:
  `Cargo.toml`, the selected `arcweft-resource-model` stanza in `Cargo.lock`;
- crate manifest:
  `crates/arcweft-resource-model/Cargo.toml`;
- shared locale owner and domain wrappers:
  `crates/arcweft-core/src/lib.rs`,
  `crates/arcweft-core/src/locale.rs`,
  `crates/arcweft-dialogue/src/character_dialogue/identity.rs`,
  `crates/arcweft-dialogue/src/tests/character_dialogue.rs`,
  `crates/arcweft-render-text/src/resolved_document.rs`,
  `crates/arcweft-render-text/tests/resolved_document.rs`;
- ordering traits required on existing closed-value owners:
  `crates/arcweft-interaction-model/src/audio.rs`,
  `crates/arcweft-layout/src/lib.rs`;
- resource production:
  `crates/arcweft-resource-model/src/lib.rs`,
  `crates/arcweft-resource-model/src/identity.rs`,
  `crates/arcweft-resource-model/src/value.rs`,
  `crates/arcweft-resource-model/src/descriptor.rs`,
  `crates/arcweft-resource-model/src/registry.rs`,
  `crates/arcweft-resource-model/src/registry/validation.rs`,
  `crates/arcweft-resource-model/src/registry/digest.rs`;
- resource tests:
  `crates/arcweft-resource-model/tests/identity_contract.rs`,
  `crates/arcweft-resource-model/tests/value_contract.rs`,
  `crates/arcweft-resource-model/tests/registry_contract.rs`;
- reconciliation:
  `docs/implementation/2026-07-20-lang-01-4-production-reconciliation.md`;
- follow-up contract already recorded:
  `docs/reviews/requests/2026-07-20-lang-01.4.2-resource-extension-manifest-wire-contract-correction.md`.

## Validation

Passed:

```bash
cargo fmt -p arcweft-core -p arcweft-dialogue -p arcweft-render-text \
  -p arcweft-interaction-model -p arcweft-layout \
  -p arcweft-resource-model
cargo test -p arcweft-core -p arcweft-dialogue -p arcweft-render-text \
  -p arcweft-interaction-model -p arcweft-layout \
  -p arcweft-resource-model
cargo check -p arcweft-core -p arcweft-dialogue -p arcweft-render-text \
  -p arcweft-interaction-model -p arcweft-layout \
  -p arcweft-resource-model --all-targets --all-features
cargo clippy -p arcweft-core -p arcweft-dialogue -p arcweft-render-text \
  -p arcweft-interaction-model -p arcweft-layout \
  -p arcweft-resource-model --all-targets --all-features -- -D warnings
git diff --check -- Cargo.toml crates/arcweft-core \
  crates/arcweft-dialogue crates/arcweft-interaction-model \
  crates/arcweft-layout crates/arcweft-render-text \
  crates/arcweft-resource-model \
  docs/implementation/2026-07-20-lang-01-4-production-reconciliation.md \
  docs/implementation/2026-07-20-lang-01-4-generic-resource-registry-substrate-cut-2a.md
```

Focused six-crate result: 320 passed, 0 failed, 0 ignored, including four
compile-fail documentation tests.

After the Lang-01.4.1 correction, the current focused result is 36 passed,
0 failed, 0 ignored, plus passing doc-tests:

```bash
cargo test -p arcweft-resource-model
cargo check -p arcweft-resource-model --all-targets --all-features
cargo clippy -p arcweft-resource-model --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

The shared locale-owner correction crosses a public dialogue/render-text
boundary, so this cut is treated as Tier 2 risk. `just test-tier2` is required
before the integration commit.

The post-AW `just test-tier2` run compiled the complete affected stack and ran
22 slow MCP checks. Five passed and 17 failed. Two failures are independent
roots: `agent_mcp_stdio_runs_agent_script` still expects `changed = false`
although the accepted transaction reports `true`, and the first native
capture fixture cannot initialize its player-backed MCP runtime. The remaining
15 failures are mutex-poison cascades after that native failure. These are the
stale Tier 2 harness cases already assigned to the launch/profile integration
owner; they are not counted as passing resource evidence and must be rerun
after that owner reconciles the fixtures.

The canonical structural audit was run:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

It scanned 3,365 files, 1,732 Rust files, 801,908 Rust physical LOC, and 93
package manifests. It reported 0 errors and 129 warnings. No resource-model or
shared-locale file produced a warning-level violation.

## Exact structure measurements

Measured from the current checkout after formatting:

| Path | Role | Bytes | Physical LOC |
| --- | --- | ---: | ---: |
| `crates/arcweft-core/src/lib.rs` | facade | 388 | 26 |
| `crates/arcweft-core/src/locale.rs` | production locale owner with unit tests | 6,717 | 218 |
| `crates/arcweft-dialogue/src/character_dialogue/identity.rs` | production dialogue identity wrapper | 5,370 | 184 |
| `crates/arcweft-dialogue/src/tests/character_dialogue.rs` | unit-test module | 33,464 | 959 |
| `crates/arcweft-interaction-model/src/audio.rs` | production audio value owner | 27,676 | 911 |
| `crates/arcweft-layout/src/lib.rs` | production layout facade/model | 29,105 | 943 |
| `crates/arcweft-render-text/src/resolved_document.rs` | production resolved-text model | 36,395 | 1,188 |
| `crates/arcweft-render-text/tests/resolved_document.rs` | integration test | 14,094 | 445 |
| `crates/arcweft-resource-model/src/lib.rs` | facade | 364 | 12 |
| `crates/arcweft-resource-model/src/identity.rs` | production identity owner | 12,011 | 421 |
| `crates/arcweft-resource-model/src/value.rs` | production typed-value owner | 23,061 | 714 |
| `crates/arcweft-resource-model/src/value/reference.rs` | production reference invariant owner | 13,984 | 388 |
| `crates/arcweft-resource-model/src/descriptor.rs` | production schema/descriptor owner | 15,454 | 573 |
| `crates/arcweft-resource-model/src/registry.rs` | production publication facade/integrity | 16,509 | 469 |
| `crates/arcweft-resource-model/src/registry/validation.rs` | production registry validation | 24,065 | 609 |
| `crates/arcweft-resource-model/src/registry/digest.rs` | production canonical digesting | 17,744 | 496 |
| `crates/arcweft-resource-model/src/retained.rs` | production retained-identity owner | 6,604 | 222 |
| `crates/arcweft-resource-model/src/canonical.rs` | production retained transcript encoder | 5,288 | 147 |
| `crates/arcweft-resource-model/tests/identity_contract.rs` | integration test | 1,917 | 46 |
| `crates/arcweft-resource-model/tests/value_contract.rs` | integration test | 8,551 | 239 |
| `crates/arcweft-resource-model/tests/registry_contract.rs` | integration test | 32,983 | 1,014 |
| `crates/arcweft-resource-model/tests/retained_identity_contract.rs` | integration test | 11,642 | 297 |

The new resource production modules are within the preferred 300–800 LOC
range, and the crate facade is 10 LOC. The touched pre-existing owner modules
remain below the audit's warning thresholds; resolved-text is the closest at
1,188 LOC.

## Deferred and blocked items

- The retained-identity value category and canonical transcript are installed.
  Built-in Image/VoiceProfile/Voice/Rig descriptor publication and accepted
  owner resolution remain in the later Lang-01.4.1 cuts recorded by the WIP
  note.
- Strict extension-manifest decoding waits for
  [Lang-01.4.2](../reviews/requests/2026-07-20-lang-01.4.2-resource-extension-manifest-wire-contract-correction.md).
  Lang-01.4 names the file and top-level intent but does not settle its exact
  tags, content keys, scalar/ID encodings, package-coordinate shape,
  duplicate rules, or source-mapped diagnostic ownership.
- Public AST/HIR/sema/project-index/tooling migration remains Cut 3.
- Bundle directory, payload locators, Agent/save/hot-reload boundaries remain
  Cut 4.
- Built-in family migration and old-family deletion remain Cuts 5–6.

## Design deviations

The package suggested semantic ownership in sema “or equivalent owner.” This
cut uses a dedicated lower-level model crate because the same exact identities,
schemas, constants, and immutable registry are future inputs to sema, bundle,
save, and Agent layers. No higher-level dependency was introduced.

The package refers generically to asset identity but the current workspace has
no common cross-family `AssetId`. The generic constant therefore preserves the
existing common `PublicId` plus exact payload kind. It is not interchangeable
with a configured `ResourceRef`.

No compatibility alias, dual reader, historical diagnostic, source gate,
source-text type parser, CSS route, or Takumi route was added.
