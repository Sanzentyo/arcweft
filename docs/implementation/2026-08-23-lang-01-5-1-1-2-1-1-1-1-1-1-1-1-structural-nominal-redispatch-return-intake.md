# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1 structural nominal redispatch return intake

Date: 2026-08-23
Inspected Git commit: `9a5d30d25620541c3f2975d31e04e04e3bc9514c`
Working tree before intake: clean; `main` matched `origin/main`

Follow-up to:
[the first rejected return intake](2026-08-23-lang-01-5-1-1-2-1-1-1-1-1-1-1-1-accepted-structural-nominal-carrier-return-intake.md).

## Intake result

- Archive safety and byte integrity: `PASS`
- Hardened request mirror: `PASS`
- Mandatory repository preflight: `FAIL`
- Required returned-archive contract: `FAIL`
- Repository/source reconciliation: `FAIL`
- Classification: `MALFORMED / DESIGN_NOT_READY`
- Further dispatch to the same repository-inaccessible executor: prohibited
- Production implementation: not authorized

This return knowingly emits the named final-contract ZIP after repository
acquisition failed. It substitutes proposed placeholder owners and a new
two-class `AcceptedRuntimeCarrier` for the required reconciliation with current
sema, core, runtime-plan, AWBC, value, snapshot, and restore authorities. It is
not an implementation authority.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract(1).zip`
- byte length: 49,327
- SHA-256:
  `7405AA33A46FDB0385E00CF29AA7BE210839068DBED18646E65103A996545EA6`

The unchanged ZIP is retained at
[`packages/zips/...final-contract(1).zip`](<../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract(1).zip>).
Its 24-file frozen mirror is retained at
[`packages/...final-contract(1)/`](<../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract(1)/README.md>).

The `(1)` suffix is the attached download name used to avoid colliding with the
first retained return. The archive's one internal wrapper exactly matches the
required output basename without that local suffix.

## Mechanical checks

- 24 safe file members under one exact wrapper;
- no absolute, drive-qualified, parent-traversal, backslash, duplicate,
  case-fold-colliding, or special Unix member;
- all 23 rows in `SHA256SUMS` match; only `SHA256SUMS` itself is not covered;
- the hardened `REQUEST.md` is exactly 10,571 bytes with SHA-256
  `E9EAD183B2BFD4D3019E8C3E51DA79136BDAE64D38AA5FE63EC4C92C1C948269`;
  it matched the hardened dispatch revision. The maintained request was later
  closed in place by the repository-local accepted design and intentionally no
  longer has those bytes; and
- archive extraction/readability completed without a CRC or member error.

## Mandatory delivery-gate failures

The return records all of the following itself:

- complete Git SHA: `UNAVAILABLE`;
- repository acquisition: false;
- Cargo metadata: unavailable;
- applicable `AGENTS.md`: none read;
- current source/search rows: zero; and
- only the failed repository-acquisition command was run.

The required `FINAL_STATUS.md`, `OPEN_QUESTIONS.md`, machine-readable final
contract, repository-aware validator, and negative mutation corpus are absent.
The manifest reports `open_questions: 0`, but that is not the required status
authority. Its requirement extractor and traceability cover only request
decision 8, while the package nevertheless asserts complete closure.

The hardened request requires the responder to stop and return only a blocker
report when the repository is unavailable. It also requires the produced ZIP
to be reopened and validated against the cited Git commit. With no cited commit
or source, the named final-contract ZIP was forbidden.

## Repository reconciliation

All claimed owner paths are placeholders such as
`crates/<runtime-owner>/src/value.rs` and
`crates/<language-owner>/src/checked/type.rs`. Current source instead has
concrete, separated authorities including core `RuntimeValue`,
`RuntimeNominalRecordLayout`/`RuntimeNominalRecordValue`,
`RuntimeVariantIdentity`, `RuntimeCheckedType`, and `RuntimeNominalTypeId`, plus
sema `AcceptedNominalId`/`AcceptedNominalCatalog` and downstream layout,
runtime-plan, AWBC, snapshot, and restore consumers.

The proposed `AcceptedRuntimeCarrier::{Structural, Nominal}` is a new parallel
algebra rather than a deletion-driven evolution of those owners. It does not
prove how sema metadata joins core executable layout without an upward
dependency, nor does it close exact accepted record/enum case ownership,
unit/tuple/record and one-field-tuple payloads, recursive generic
instantiation, Result/Option predicates, or stale world/generation rejection.

The package also invents a persistence record and coordinator publication
model without identifying the current tag allocator or snapshot authority. Its
own wire sketch starts with fixed `01 00` version bytes and leaves key/tag
owners abstract, contrary to the hardened request's exact version-1 canonical
varint/golden-byte gate. It proposes temporary compatibility constructors and
a versioned compatibility decoder despite the unreleased no-legacy rule.

No returned API, carrier, wire grammar, or restore model is safe to reuse
independently. Generic principles such as typed fail-closed admission and
two-phase validation were already accepted substrate and do not constitute new
closure credit.

## Next action

Do not modify or redispatch the maintained structural nominal request again.
The request already required the behavior this return ignored. It is now
resolved by the
[repository-local accepted design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/README.md),
which begins from the actual sema/core/runtime-plan/AWBC/value/snapshot owners
and preserves the current layer direction.

No Rust, Cargo, generated production artifact, fixture, or runtime test was
changed or run for this rejected design-only intake.
