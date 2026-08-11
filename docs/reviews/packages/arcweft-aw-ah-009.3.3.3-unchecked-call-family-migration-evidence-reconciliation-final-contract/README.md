# AW-AH-009.3.3.3 unchecked call-family migration evidence reconciliation

Status: `READY_FOR_IMPLEMENTATION`

This archive is the focused final-contract correction for AW-AH-009.3.3
section 19. It makes the final migration evidence truthful without changing
Arcweft production semantics. It does not contain a Rust patch, repository
overlay, manifest edit, test edit, schema edit, fixture edit, or stable design
chapter edit.

## Repository and dispatch basis

- Repository: `Sanzentyo/arcweft`
- Inspected Git `main`: `5f33ea20fcde7317332c95324701ed4ea7ab813a`
- Dispatch-provided Jujutsu change: `yxvlsqorouqlolxvwtltxltmtqutsxku`
- Request SHA-256:
  `c2a101e93213682b8d05e7f08b2fe58cf8c187e6e6f25129b0513941c2e05b2`
- Parent AW-AH-009.3.3 package SHA-256:
  `9d1f989f5e0e698aeff1098dd7ecee7e01a66616a00a0571ee333a3b1b7ddc78`
- AW-AH-009.3.3.1 package SHA-256:
  `3d81158eb37f503ef7b0f242a79015ba1ab00e3954a8dae4384f45eaab55b672`
- AW-AH-009.3.3.2 package SHA-256:
  `c5b6bbf9addb45f2d6ecbdfd8f2abc4d6602f079a847a20db8f26140d53a248f`
- Implementation note inspected at Git blob:
  `2a2c861eeb059f499712f385881d067b95936d98`

The three consumed ZIPs passed decompression/CRC checks. Their internal
manifests were independently checked against extracted member bytes, including
the parent/external all-zero self-entry convention and the curried package's
manifest-excluded convention.

## Final decision

`CallableFamily::ALL` contains 23 entries in current production. For section 19
migration evidence they are classified exactly once as follows:

- 20 `RejectingSchema` families: one clean accepted case and one genuine
  rejected-or-poisoned case are mandatory;
- 3 `IntentionallyUnchecked` families — `Drop`, `Promotion`, and `Speaker`:
  one clean accepted case and one clean-recovery case are mandatory.

A clean-recovery case resolves the same family candidate, checks every authored
recovery expression exactly once without an expected type, retains the
candidate and documented result, emits no callable argument diagnostic, and
remains `CallPoison::Clean`. A normal expression-recovery diagnostic may still
exist outside the callable fact; it is not relabelled as a family rejection.

Unknown targets, non-callable targets, unsupported surfaces, and terminal query
errors are separate dispositions. None may satisfy a family negative row.

The closed matrix therefore has 46 family cases:

```text
23 accepted
20 rejected-or-poisoned
3 clean-recovery
```

## Normative precedence

This archive replaces only AW-AH-009.3.3 `TEST_MATRIX.md` section 19 and the
traceability rows that quote its universal accepted-and-rejected quantifier.
It also updates the section-19 audit cardinality to current
`CallableFamily::ALL == 23`, including `StageMethod`.

Everything else in the accepted AW-AH-009.3.3 package remains normative.
AW-AH-009.3.3.1 remains authoritative for curried group validation; a curried
candidate reports its base family and creates no 24th family. AW-AH-009.3.3.2
remains authoritative for typed external project binding paths; the `Project`
case must use that typed publication path and may not reconstruct identity from
text.

No wording in this correction authorizes a second resolver, old dispatcher,
source scan, test-only production semantic branch, compatibility path, source
gate, removed Dialogue syntax, fake Dialogue `Expr::Call`, second expression
arena, CSS path, or Takumi path.

## Read order

1. `FINAL_CORRECTION.md` — exact replacement contract for section 19.
2. `FAMILY_CLASSIFICATION.md` — all 23 families and the closed classification.
3. `TEST_MATRIX.md` — executable case model, cases, counters, and drift tests.
4. `REQUIREMENTS_TRACEABILITY.md` — request requirement to decision/test map.
5. `REPOSITORY_EVIDENCE.md` — inspected source/package evidence and limits.
6. `FINAL_STATUS.md` — readiness declaration.
7. `OPEN_QUESTIONS.md` — exactly `none`.
8. `MANIFEST.txt` — exact hashes and lengths for every other member.

## Verification boundary

The design inputs, package hashes, package member manifests, Git revision,
current typed family/schema/fact sources, archive contents, archive manifest,
ZIP decompression, clean extraction, and outside ZIP SHA-256 are verified in
this artifact runtime. Production Rust was not modified, so no Cargo, Clippy,
Tier 2, or structure-audit execution is claimed as newly run. Repository-recorded
historical validation is reported only as historical evidence.

The Git commit was inspected through the connected repository. The Jujutsu
change ID is the exact dispatch-provided identity; the connector does not expose
an independent Jujutsu workspace view, so this archive does not claim to have
enumerated an uncommitted JJ diff.
