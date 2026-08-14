# External-lowering/generation-admission return intake and residual blocker

Date: 2026-08-14

Supersedes the active blocker state in:
`docs/implementation/2026-08-14-checked-value-root-site-return-invalid.md`

Inspected Git baseline:
`b1bf20910643206b75a315aba70f5ec468c03612` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake.

## Returned archive intake

The downloaded archive had a delivery-only `1-` prefix. Because the canonical
repository name was free, intake removed that prefix. The retained archive is:

`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1.1.1-external-lowering-and-independent-generation-admission-authority-correction-final-contract.zip`

SHA-256:
`b1bd2e49c2d9a7859e12a22d85971aa9f9ad092600e05a2dc55c144d63c5056d`

The 189,964-byte ZIP contains 89 files without a redundant wrapper. It has no
unsafe/rooted/drive/traversal path, symlink/reparse entry, or case-insensitive
collision. All 89 extracted files match their ZIP-member SHA-256 values and
all 88 internal `MANIFEST.sha256` rows pass. `SOURCE_REQUEST.md` is
byte-identical to the maintained request, SHA-256
`2498106d805515f2fba326ef55685a8699aec2ab1abb986e22bc2f0a1f984cc6`.

The package reports `READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`, evidence
commit `80348beed0efa72db07f712122217b4e679e0a97`, and all Arcweft-owned
versions fixed at `1`. Its 12 decisions, 14 phases, 836-row inventory,
2,021-row test matrix, and 23-row source-evidence set were inspected. Of the
23 source rows, 21 still match the current baseline. The two changed files are
the independently accepted generation-root scalar and exhaustive value-shape
cuts that landed after the package evidence commit; they do not cause the
blockers below.

## Readiness adjudication

The return materially and correctly fixes the prior public cross-crate
construction, raw self-admission, opaque, AudioCommand, operational-expression,
and contradictory-test decisions. Nevertheless, full-package/current-source
inspection and an independent Sol max audit classify it as
`INVALID_AS_DELIVERED`, not `READY_FOR_IMPLEMENTATION`. The residual defects
are current-Git-resolvable internal design gaps rather than an external
authority, so `NOT_READY` is not the repository classification.

1. P02 migrates `final_expr` and `final_pattern` to facts containing
   `RuntimePlanTypeId`, but the only legitimate ID issuer is
   `RuntimePlanBuilder::push_type`, introduced in P03. Because the ID has no
   public constructor, the stated P02 cannot compile. P08 similarly adds
   inherent methods on `AdmittedRuntimeProduct`, introduced only in P10.
2. The package assumes the final lowerer receives an accepted semantic type
   for every expression node. Current `RuntimePlanSemanticFacts` contains
   selected expression facts but no exhaustive `ExprId` type table, while the
   complete `CheckedExpression` facts exist earlier in
   `FinalSemanticAnalysis`. No exact projection, ownership, snapshot
   correlation, or error mapping carries them to `FinalExprLowerer`.
3. The package describes the admitted generation as independent and
   non-forgeable, but exposes public constructors for every projection row and
   builder, a public fact-section decoder, and public
   `AdmittedRuntimeGeneration::try_issue`. Non-Serde data and absence of a raw
   conversion do not prove accepted-world provenance. The final contract must
   either define this as a trusted-integrator structural boundary and narrow
   its claims, or provide a real layer-correct issuance capability and specify
   how compiler and verified-bundle inputs obtain it.
4. Three construction surfaces remain incomplete. The compiler cannot build
   private-field `RuntimeNominalRecordFieldProjection` rows; the referenced
   `RuntimePatternBindingCoordinate` has no definition; and the retained AWBC
   nominal-record-domain table has no builder method, program accessor, or
   final wire/admission surface.
5. An exhaustive source `ExprId` type table would still not cover final
   lowering's synthetic nodes. The return does not identify the accepted type
   source for reduction/agent/assignment scaffolding or synthesized
   empty/composite expressions, so their semantic identity and
   checked/operational classification remain underdetermined.

## Accepted and blocked scope

The returned effect-owned AudioCommand coordinate, atomic opaque validation,
checked/operational type split, public checked raw-construction principle,
generation-first load order, and corrected tests are retained design inputs.
Current main's lower vocabulary, catalog digests, ownership path, value shape,
and project/producer scalar projections remain accepted.

Do not implement generation issuance, complete typed-expression lowering,
plan/AWBC admission, context/domain issuance, operational publication, or
bundle/restore migration until the child correction closes the fact and
provenance boundary. Small lower substrate may proceed only when it does not
commit to the blocked issuer or phase order.

Child correction request:
`docs/reviews/requests/2026-08-14-lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-and-compile-clean-admission-order-correction.md`

## Validation performed

- source ZIP SHA-256 and byte length: verified;
- unsafe path, traversal, symlink/reparse, and case-collision preflight: passed;
- ZIP member versus extracted file SHA-256 parity: 89/89 passed;
- internal `MANIFEST.sha256`: 88/88 passed;
- request-copy SHA-256 equality: passed;
- all normative decisions, APIs, mapping tables, metadata, inventory, tests,
  implementation order, and repository evidence were inspected;
- focused test-matrix scans confirmed the returned Option `None`, atomic
  opaque, effect-owned AudioCommand, and operational-root corrections; and
- no production code was changed as part of this intake adjudication.
