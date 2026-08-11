# Required implementation test contract

Tests use public/typed APIs and structured dependency metadata. No test may read implementation source to look for symbol spellings, module paths, or deleted syntax.

## Positive and canonical fixtures

| ID | Test |
| --- | --- |
| P-001 | bundled minimal input decodes; canonical bytes equal `examples/minimal.canonical.json` |
| P-002 | bundled full input decodes; canonical bytes equal `examples/full.canonical.json` |
| P-003 | canonical minimal/full decode and re-encode byte-for-byte unchanged |
| P-004 | decode(input) -> encode -> decode produces semantic equality including source-independent typed values |
| P-005 | every scalar type and scalar constant variant is covered exhaustively |
| P-006 | every `ResourceValueType` variant is covered exhaustively |
| P-007 | every `ResourceConstValue` shape is covered: option none/some, empty/nonempty list, map, record, enum unit/payload, three reference categories |
| P-008 | all seven retained categories, both presentation scopes, and scroll region owner are covered |
| P-009 | every `LayoutUnit`, exposure, hot-reload, field-presence, bound-kind, schema-kind variant is covered |
| P-010 | locale noncanonical valid input canonicalizes through current `LocaleId` and regenerates canonical casing |
| P-011 | subnormal/normal/positive-zero/max-finite float bits round-trip exactly |
| P-012 | one document publishes multiple resource types and forward references resolve |
| P-013 | same-package references across candidate ordering resolve after aggregate publication |
| P-014 | selected cross-package dependency reference resolves only when dependency manifest/base target is present |

## Determinism

| ID | Test |
| --- | --- |
| D-001 | permutations of schemas/types/codecs regenerate identical canonical bytes and registry digest |
| D-002 | permutations of fields/variants/codec versions/record fields/map entries regenerate identical bytes |
| D-003 | list/sequence permutations produce different semantics/bytes, proving semantic order retention |
| D-004 | docs/provenance changes leave descriptor and registry semantic digest unchanged where current invariant says so |
| D-005 | semantic descriptor change changes independently recomputed descriptor digest |
| D-006 | canonical bytes produce frozen RawDigest vectors on every platform |
| D-007 | bundle manifest-set section bytes and AWFB content digest are input-order independent |
| D-008 | diagnostic ordering is independent of candidate insertion order |

## Strict decoder negatives

Machine vectors are in `vectors/negative-cases.json`.

- missing/malformed/unsupported format and schema version;
- duplicate keys at root and every nested object;
- missing field, explicit null, wrong root/field shape, unknown field;
- unknown enum tag and forbidden/missing tag content;
- malformed/overflow integer and `-0` integer;
- NaN, positive/negative infinity, malformed float text, and negative zero bits;
- unpaired surrogate, wrong char scalar count, malformed typed IDs/digests/locale;
- attempted `bytes` tag/base64/hex byte encoding rejected as unknown;
- duplicate package/schema/type/codec/field/variant/map/record identities;
- required field default, empty nonempty-list, constraint mismatch/inversion/empty interval;
- unknown or wrong-kind schema, unknown resource-ref type, body mismatch, missing/unsupported codec, invalid capabilities;
- forged descriptor digest;
- package coordinate mismatch, version conflict, unresolved dependency;
- retained/reference cross-category and wrong retained kind;
- nested default range assertions including related type/first occurrence ranges.

## Limits and atomicity

For each bytes/depth/nodes/string/array/object/semantic-record/work limit:

1. exactly-at-limit succeeds when semantically valid;
2. one-over returns the exact limit code;
3. no accepted document is returned;
4. no aggregate registry replaces the supplied base `Arc`;
5. no canonical cache entry or bundle section is produced.

Include aggregate tests where each individual document is within per-document limits but the selected package set triggers duplicate/version/registry failures; publication remains atomic.

## Descriptor and artifact tamper tests

| ID | Test |
| --- | --- |
| T-001 | change descriptor field, keep old claim -> descriptor mismatch |
| T-002 | change claim only -> descriptor mismatch |
| T-003 | docs/provenance-only change with recomputed canonical manifest -> descriptor claim unchanged, manifest RawDigest changed when docs wire changes |
| T-004 | embedded manifest raw digest tamper -> artifact digest mismatch before JSON decode |
| T-005 | valid noncanonical embedded JSON -> artifact noncanonical manifest |
| T-006 | reorder artifact entries -> artifact malformed/noncanonical ordering |
| T-007 | final registry digest tamper or engine-base mismatch -> registry digest mismatch |
| T-008 | section trailing bytes/truncated length/count overflow -> artifact malformed |
| T-009 | unknown required code 22 on an old/fixture reader -> required-section rejection, not skip |

## Actual package/build integration

- root `resource-type-manifest` path resolves through existing normalized path/containment APIs;
- absent optional field performs no filesystem probe;
- dependency manifest comes only from explicit resolver seed;
- directory containing a conventionally named manifest but no declared/seeded path is ignored;
- malformed UTF-8 is rejected at loader byte boundary with exact offset;
- loaded topology retains source-backed manifests and one published `Arc<ResourceTypeRegistry>`;
- compiler receives that same registry object/digest through existing project registration;
- bundles with extensions contain required section code 22; bundles without selected extensions omit it;
- runtime reconstructs against the supplied engine base and validates the final digest;
- no permissive fallback to empty/base-only registry when the section is missing/invalid but extension IDs are referenced.

## Architecture evidence

Use tests that compile against public types and a structured `cargo metadata` assertion over package dependency edges. Explicitly forbid tests such as `include_str!("../src/...")`, repository path scans, regex symbol searches, or file-name assertions.

## Required implementation commands

From repository root after implementation:

```sh
cargo fmt --all -- --check
cargo clippy -p arcweft-resource-model --all-targets -- -D warnings
cargo clippy -p arcweft-resource-manifest --all-targets -- -D warnings
cargo clippy -p arcweft-launch --all-targets -- -D warnings
cargo clippy -p arcweft-project-loader --all-targets -- -D warnings
cargo clippy -p arcweft-bundle --all-targets -- -D warnings
cargo test -p arcweft-resource-model
cargo test -p arcweft-resource-manifest
cargo test -p arcweft-launch
cargo test -p arcweft-project-loader
cargo test -p arcweft-compiler
cargo test -p arcweft-bundle
cargo test -p arcweft-runtime-driver
cargo metadata --format-version 1 --no-deps > target/resource-manifest-metadata.json
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run the repository structural audit required by `AGENTS.md` for the new crate/dependency/public-contract cut and store the structured report with implementation evidence.
