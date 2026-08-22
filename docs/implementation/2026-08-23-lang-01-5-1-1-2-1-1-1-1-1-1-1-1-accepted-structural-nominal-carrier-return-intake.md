# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1 accepted structural nominal carrier return intake

Date: 2026-08-23
Inspected Git commit: `9168c8ac7285c6b44f29018626a0e7c1b0059796`
Working tree before the three-package intake: clean; `main` matched
`origin/main`

## Intake result

- Archive member safety: `PASS`
- Required wrapper/package contract: `FAIL`
- Repository/source reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: blocked
- Next action: dispatch the in-place hardened request with current repository
  access and this rejection evidence

The archive proposes a new carrier around an invented runtime crate and type
catalog without inspecting current Arcweft owners. It does not close the eight
requested decisions and must not replace Cut 2's typed fail-closed boundary.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract.zip`
- byte length: 21,700
- SHA-256:
  `8D447DA28397459390271C05BA0BF63476F6B98DA90602FB05699B404E2DB89D`

The unchanged ZIP is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract.zip).
The ZIP used the wrong top-level wrapper
`arcweft-accepted-structural-nominal-runtime-carrier-final-contract`; its
11-file frozen content mirror is retained under the required package basename
at
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract/00-README.md).

## Integrity checks passed

- 11 safe file members and 39,842 uncompressed bytes;
- no absolute, drive-qualified, parent-traversal, backslash, duplicate,
  case-fold-colliding, or special Unix member;
- all ten `07-package-manifest.sha256` rows match; and
- `REQUEST.md` SHA-256
  `CBE6A1F1F20F2C5C11DF678B8098165CE8931820ECE459C7BF1CF203BE7BC5A4`
  exactly matches the pre-hardening maintained request revision at the
  inspected commit. This historical hash is not the next-return acceptance
  hash.

## Required package failures

The wrapper name does not match the required archive basename. The package
also lacks `FINAL_STATUS`, `OPEN_QUESTIONS`, a repository-aware validator,
negative self-tests, the required machine-readable manifest shape, owner/
consumer and dependency matrices, and canonical transcript fixtures.

Repository, SHA, checkout, `AGENTS.md`, source symbols, commands, and the
accepted predecessor ownership matrix are all recorded as unavailable. The
output-name audit found zero required-output sections. Traceability extracted
only three generated fragments from implementation-order, tests, and archive
sections; it did not cover decisions 1–8.

## Result-changing source contradictions

### It selects nonexistent owners

The return places `RuntimeValue` in
`crates/arcweft-runtime/src/value.rs`, but no `arcweft-runtime` crate exists;
the production value owner is `arcweft-core::value::RuntimeValue`. It invents
`AcceptedNominalTypeId` and `AcceptedStructuralLayoutId`, then passes an
`AcceptedNominalCatalog` into the core-side constructor without reconciling
that the actual catalog is sema-owned and core cannot depend upward on it. It
also ignores the current `RuntimeNominalTypeId`, `TypeLayoutHash`,
`RuntimeCheckedType`, plan type table, record/variant domains, and AWBC value
owners.

Adding the proposed `RuntimeValue::AcceptedStructuralNominal` now would create
a second nominal/layout algebra beside the accepted schema projections rather
than making those sole owners constructible.

### It does not model the requested structural families

The single `{ nominal, layout, fields: Box<[RuntimeValue]> }` shape does not
define:

- distinct accepted record and enum-case ownership;
- unit, tuple, record, and exact one-field tuple enum payload rules;
- stable field and case identities/ordinals;
- recursive/mutually recursive generic instantiation;
- Result/Option-shaped checked predicates;
- the metadata-to-executable-layout catalog join; or
- stale-world/generation validation across cross-crate consumers.

It therefore cannot be used as the canonical live carrier, Match projection,
value digest, or snapshot representation.

### The wire/version design is not repository-derived

The package invents a per-value explicit `u16le` version and leaves the runtime
value tag to a future max-plus-one allocation without inspecting the owner tag
table. It also describes legacy acceptance, migration, and opaque future
forwarding despite the request's no-version-bump/no-legacy/no-migration rule.
No golden bytes exist because no tag or current identity codec was verified.

## Correct resend

Do not implement this return and do not create a compatibility carrier around
it. Do not create a new child request and do not resend the rejected request
bytes. Dispatch the same request lineage, revised in place at
[`Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction.md)
with:

- byte length `10,571` and SHA-256
  `E9EAD183B2BFD4D3019E8C3E51DA79136BDAE64D38AA5FE63EC4C92C1C948269`;
- a clean current `main == origin/main` checkout whose full dispatch SHA is
  frozen in the response evidence; and
- this intake as mandatory rejection evidence.

The next archive's `REQUEST.md` must match those hardened bytes. It must begin
from the actual sema/core/runtime-plan/AWBC owners and either enrich the one
legitimate model or remain fail-closed. The responder must withhold the named
final-contract ZIP if repository preflight, any required member, traceability,
manifest, wrapper, repository-aware validation, negative self-test, or
reopened-ZIP validation fails. A blocker report is correct; a known-failed
“final contract” is not.

No Rust, Cargo, generated artifact, fixture, or runtime test was changed or run
for this design-only intake.
