# Lang-01.1.1.3.1 checked callable catalog return intake

Date: 2026-07-25

## Package verification

The returned archive is retained at:

- [`Lang-01.1.1.3.1 checked callable catalog reconciliation`](../reviews/packages/arcweft-lang-01.1.1.3.1-checked-callable-catalog-authority-and-consumer-scope-reconciliation-final-contract.zip),
  SHA-256
  `0E4746ABD1589F0228ADC62A074DFC07EEC92F3DF4DBC432E58138FD21500F4C`.

The ZIP has 12 members. Its filename-sorted `MANIFEST.sha256` has 11
non-self rows; every recorded SHA-256 and byte length matches the corresponding
member. `OPEN_QUESTIONS.md` is exactly `none`, and the embedded status is
`READY_FOR_IMPLEMENTATION`.

`README.md` and `FINAL_STATUS.md` say that all 12 non-self manifest rows were
verified. The archive actually has 12 total members and 11 non-manifest
members. This is an evident count typo rather than a content, hash, precedence,
or semantic ambiguity. Intake records the correct count and does not request a
redelivery for it.

The package inspected pushed Git commit
`b305c698b22a01b30f1d7e68be6d925e6e3a2875`. Current implementation must use
the current pushed owners and treat that commit only as the package's evidence
baseline.

## Intake status

`RETURNED_CATALOG_AUTHORITY_ACCEPTED_NARROW_TRAIT_VALIDATOR_CORRECTION_REQUIRED`.

The package closes the catalog gap that blocked Lang-01.1.1.3. These decisions
are accepted and must not be requested again:

- `RegisteredCallableCatalog` remains the sole accepted metadata catalog;
- each checked fact retains the exact accepted `Arc<CallableRecord>`;
- `CheckedCallableCatalog` owns checked-only identity, execution, effects,
  conformance, closure, source-index, and derived-interface facts;
- fixed environment/standard rows remain on the accepted record;
- source-callable rows exist only in checked facts;
- project/LSP/Agent/persistent/runtime consumers share the same checked
  generation and retain typed IDs or derived output rather than metadata
  copies; and
- checked/runtime/persistent identities, stale failure, transaction order, and
  the deletion-driven consumer switch follow the returned contract.

No copied metadata catalog, trait-only signature catalog, compatibility view,
dual reader, source gate, removed-syntax diagnostic, CSS path, or Takumi path
is authorized.

## Remaining pre-check validator identity gap

The current accepted signature schema stores
`CallableValidator::Trait(TraitCallableId)`. That schema is frozen into the
accepted `CallableRecord` before body checking or checked-ID/conformance
construction. The returned package nevertheless requires deletion of both
`TraitCallableId` and `CallableCandidateId::TraitMethod` without defining the
replacement pre-check validator payload.

The gap is observable beyond one enum field:

- the trait resolver synthesizes `TraitCallableId` before creating the schema;
- `CallableValidator::Trait` participates in schema digesting, migration
  evidence, and registered-call selection;
- ambiguity diagnostics retain `Arc<[TraitCallableId]>`; and
- AW-AH-009.3 tests observe `CallableFamily::TraitMethod` for resolver work
  accounting and signature help, while the returned final candidate becomes
  `CallableCandidateId::Project(CallableDeclarationKey)`.

A checked ID cannot fill the accepted-schema slot because it is constructed
after catalog freeze. A display name, local implementation index, sentinel, or
hidden retained legacy variant would violate the returned deletion contract.
Selecting the final typed payload and the family projection is therefore a
small public semantic-owner decision, not an implementation detail.

The independently throwable correction is:

- [Lang-01.1.1.3.1.1 trait validator and resolver-family identity reconciliation](../reviews/requests/2026-07-25-lang-01.1.1.3.1.1-trait-validator-and-resolver-family-identity-reconciliation.md).

## Deletion-driven implementation disposition

Do not repair, extend, alias, or add new consumers to `TraitCallableId`. The
type, old candidate variant, old validator payload, old ambiguity payload, and
any obsolete signature origin must be deleted with their selected structural
replacement in one compiling authority switch after `.3.1.1` returns.

Privacy reductions that expose no new authority, such as making the raw record
or legacy-ID constructor crate-private when all callers are already internal,
may be separated later as coherent cleanup. They are not a substitute for the
missing final owner and are not mixed into the active Proof/Asset cut.

`FunctionKind` is already deleted and must not be restored. The package's
accepted-record, checked-catalog, consumer, effect, and runtime decisions remain
implementation work at the established Lang-01.1.1 dependency position; they
are not permission to run ahead of the active Proof and RichText slices.

## Production boundary

The full Lang-01.1.1.3 public switch remains design-blocked only at the narrow
trait-validator/family boundary above. The returned `.3.1` package otherwise
replaces the former catalog-authority blocker. This wait does not block the
current Proof-first dependency order or deletion of independently obsolete
producers whose final owners are already specified.
