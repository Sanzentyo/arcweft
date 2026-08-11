# Lang-01.5.1.2.1 final contract

**Status:** `FINAL_CONTRACT`  
**Repository:** `Sanzentyo/arcweft`  
**Pinned `main`:** `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`  
**`OPEN_QUESTIONS`:** `0`  
**`fallback`:** `false`  
**Production code changed by this delivery:** `false`

This archive is the independently usable final contract for reconciling the
Lang-01.5.1.2 typed content-root admission design with the selected
Lang-01.3.1 removal of the author-facing `source` declaration, public
`Source<T, E>`, and `EntityKind::Source`.

The contract is final rather than a fallback. It preserves the already selected
binary topology, binary overlay, `CharacterPackage`, source-provenance,
presence, transaction, and topology-revision substrate. It changes only the
parts whose authority depended on a retained Source root or source-owned
`content` declaration, plus the consumer projections that necessarily follow
from that removal.

## Normative result

The closed content-root inventory is:

1. file-backed Character packages;
2. authored entities of exactly `Flow`, `View`, `Action`, `Activity`, `Asset`,
   `Signal`, `Metric`, or `Layer`; and
3. exact accepted configured-resource declarations.

There is no Source root family and no callable root family. An ordinary
function, authored Stream generator, Stream passthrough function, or external
capability function is never admitted merely because its return type is
`Stream<T, E>` or because its execution mode is generator/external.

A removed Source reference is resolved through the ordinary final typed
resolver and receives an ordinary unresolved/wrong-target result. No
`SourceRemoved`, migration-only, alias, dual-family, or compatibility
diagnostic is introduced.

## Package map

- `FINAL_CONTRACT.md` — authority, invariants, and completion boundary.
- `CONTENT_ROOT_FAMILIES.md` — exact closed family and resolution precedence.
- `RUST_SHAPES.md` — normative Rust ownership and API shapes.
- `REVISION_AND_ADMISSION.md` — topology transcript, presence, and atomic order.
- `PROJECT_INDEX_AND_CONSUMERS.md` — ProjectIndex, bundle, watch, and LSP projections.
- `DIAGNOSTICS.md` — deterministic ordinary diagnostic ordering and evidence.
- `DELETION_AND_IMPLEMENTATION_ORDER.md` — Source/content deletion inventory and cut order.
- `TEST_MATRIX.md` — positive, negative, revision, consumer, and transaction tests.
- `NORMATIVE_DELTA_LANG_01_5_1_2.md` — row-by-row amendment of the returned package.
- `COMPATIBILITY_AND_NON_GOALS.md` — explicit no-compatibility statement.
- `REQUIRED_DECISIONS.md` — closed decision ledger.
- `REPOSITORY_AWARE_VALIDATION.md` — exact repository and package validation scope.
- `machine/CONTENT_ROOT_CONTRACT.json` — machine-readable family/result summary.
- `evidence/SOURCE_INVENTORY.csv` — repository paths and inspected blob identities.
- `REQUEST_SPEC.md` — exact supplied request.
- `PACKAGE_STATE.json`, `OPEN_QUESTIONS.txt`, `MANIFEST.txt`, and
  `SHA256SUMS.txt` — package state and integrity.

## Authority and use

Normative conflicts are resolved in this order:

1. `FINAL_CONTRACT.md`;
2. `CONTENT_ROOT_FAMILIES.md`, `RUST_SHAPES.md`, and
   `REVISION_AND_ADMISSION.md`;
3. `PROJECT_INDEX_AND_CONSUMERS.md`, `DIAGNOSTICS.md`, and
   `DELETION_AND_IMPLEMENTATION_ORDER.md`;
4. `TEST_MATRIX.md`;
5. explanatory/evidence files.

`NORMATIVE_DELTA_LANG_01_5_1_2.md` defines exactly which rows of the prior
Lang-01.5.1.2 package are amended. Every unamended safe-substrate row remains
authoritative.

## Verification boundary

The contract was reconciled against the pinned repository through the private
GitHub connector, including the latest `AGENTS.md`, manifest/source-map owners,
the implemented binary topology and Character package substrate, the current
prefix-based loader gap, the final symbol/nominal substrate, retained/resource
identity owners, ProjectIndex, and the repository intake ledger for the prior
ZIP.

No checkout was available in this environment and no production file was
modified. Consequently no Cargo command is represented as newly executed by
this delivery. The archive itself was deterministically built and independently
validated for entry uniqueness, path containment, manifest/hash agreement,
machine-state agreement, forbidden fallback state, forbidden legacy positive
API shapes, and ZIP readability. Exact details are in
`REPOSITORY_AWARE_VALIDATION.md` and `verification/package_validation.log`.
