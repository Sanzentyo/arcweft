# Lang-01.5.1.1.2.1.1.1.1.1.1.1.3 task-plan semantic seal return intake

Date: 2026-08-22
Inspected Git commit: `f43ca943d84f9a6a6da17605947a3d30c518a5a8`
Working tree before intake: clean; `main` matched `origin/main`

## Intake result

- Archive safety and integrity: `PASS`
- Internal package validator: `PASS`
- Internal negative self-tests: `PASS` (14 rejection cases)
- Repository reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: `BLOCKED`
- Open questions claimed by the package: none
- Production Rust, Cargo, generated artifacts, and fixtures changed by this
  intake: none

The return correctly removes the task-plan self-digest cycle and selects the
core/bundle dependency direction, but it is not implementation-ready against
current `main`. It assumes a completed Cut 3 View site/admission product that
was deliberately removed, and its executable child transcripts require
semantic inputs which neither current owners nor a defined same-cut owner can
construct.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip`
- byte length: 86,257
- SHA-256:
  `9A201483978DBBF060145E31638364FFFEAB64836589139F69124103CEC1BEDE`

The unchanged byte authority is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip).
Its 37-file byte-identical frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract/README.md).

## Performed and passed

- Recorded the exact clean `main`/`origin/main` state and full Git SHA before
  intake.
- Verified one exact top-level wrapper, 37 file members, 210,290 uncompressed
  bytes, no absolute/drive/parent-traversal path, no duplicate member, no
  case-fold collision, and no special Unix file type.
- Verified the retained ZIP is byte-identical to the external attachment.
- Independently verified all 34 `MANIFEST.json` payload rows by byte length and
  SHA-256, with no missing or extra payload.
- Verified manifest SHA-256
  `CA6E0159CABEED948D6EF89F16109883804CFA6D4AD78B0C5AB64EE7EFE62E7A`
  equals `MANIFEST.sha256`.
- Verified the embedded `inputs/CURRENT_REQUEST.md` SHA-256
  `95525F7E29AFA995B08A3457EF3A79ED3100398C10BDDB6985232245FEFFB3BC`
  exactly matches the maintained request.
- Inspected the Python validators before execution. Their writes are limited
  to temporary mutation fixtures; repository mode uses read-only Git and
  Cargo metadata commands.
- Ran the extracted-directory and retained-ZIP validators with
  `uv run --no-project`; both reported `PASS`.
- Ran `tools/negative_self_tests.py` with `uv run --no-project`; all 14
  mutations were rejected.
- Obtained an independent Sol-max semantic audit and assigned Sol max the
  correction design itself. The resulting blocked design is retained under
  [`docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1-current-runtime-plan-semantic-owner-and-view-predecessor-reconciliation/`](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1-current-runtime-plan-semantic-owner-and-view-predecessor-reconciliation/README.md).
- Ran an independent Luna-max mechanical audit of links, paths, status
  consistency, retained ZIP/mirror/checksums, naming, whitespace, and
  production-file absence; all checks passed after removing three trailing
  spaces from design metadata.
- Enumerated the 59 retained review ZIPs. The sorted
  `name<TAB>length<TAB>sha256<LF>` transcript hashes to
  `E4C507D4A4ACCAB241EA80767935C17CC9B7EF8146C7F907D1E81347402932BF`.

## Performed but blocked or failed

The package's repository-aware validator is pinned to its inspected commit
`515bb071437c3af053f1560c3119906dc8002efc`. Running it against current `main`
failed with the expected exact-HEAD mismatch. No second checkout, branch,
worktree, or workspace was created to reproduce that obsolete baseline.

Current repository reconciliation failed for the result-changing reasons
below.

### The View predecessor does not exist yet

The return's compile-clean sequence says Cut 3 publishes a stable
`ViewMatchSiteId` and exact `CheckedViewMatchAdmissionDigest`, and its final
binding joins that row into `ValidatedViewProgramResource`. Current accepted
implementation evidence says the opposite: the site constructor was removed
because an ordinary function Match plus an arbitrary View program could mint a
false site. Current Cut 3 is a safe subset and publishes neither type/product.

The maintained order is:

1. request `.1.2` closes generic Match and the View declaration/body semantic
   path;
2. request `.1.4` closes retained View operations, slots, captures, site, and
   admission;
3. this task-plan child consumes those actual products; and
4. Cut 5 publishes the public task/runtime/persistence switch.

The return inspected neither the current Cut 3 implementation note nor request
`.1.4` and cannot replace their open decisions with package placeholders.

### The executable transcript has no constructible current input

The claimed fifteen-table transcript names semantic roles absent from the
actual plan rows:

- `RuntimeLocalDeclaration` contains only its plan type, not the returned
  storage/mutability, initialization, or owner coordinate roles;
- `RuntimeNominalRecordDomainField` and `RuntimeVariantCase` contain source
  names and plan types, not accepted field/case semantic identities;
- `RuntimeFunctionSite` contains parameters, captures, and a body, but no
  accepted function semantic identity, declared modes, return type, endpoint
  inventory, or effect contract;
- current `HostTaskRequestTemplate` is capability, operation spelling, and
  expression arguments, while the returned `RuntimeHostTaskRequestTemplate`
  and its endpoint/accepted-field/role-path rows are schema placeholders; and
- `RuntimeControlEffectContract` and
  `RuntimeControlEffectContractId` do not exist as current accepted owners.

The archive defines bytes for these hypothetical rows but does not define the
typed sema/lowering publication that constructs them or the deletion/migration
of current name-only rows. Implementing the transcript would therefore require
guessing semantic evidence, hashing excluded source spelling, or creating a
parallel side table. All three violate repository policy and the request.

### The upper seal cannot be finalized ahead of `.1.4`

`ValidatedViewTaskPlanBinding` is specified in terms of
`CompilerLocalViewMatchCatalogRow`, `ViewMatchSiteId`, and
`CheckedViewMatchAdmissionDigest`, all of whose result-changing construction
and executable-consumer decisions are owned by open request `.1.4`. The
task-plan digest can retain the accepted high-level rule that View contributes
program/site/admission and excludes accepted revision, but its production join
and error precedence cannot be declared complete before those actual owners
exist.

The returned Rust schema also makes bundle accept the compiler-local row type,
which would add a forbidden `bundle -> compiler` edge. The compiler is the
existing legal orchestrator and must project the local row into actual shared
core/View types before calling bundle.

### Construction and sealing APIs are unreachable

The returned `RuntimeTaskPlan` has private fields and no seed or validated
constructor, yet `push_runtime_task_plan` requires callers in runtime-plan to
provide the final row. The proposed numeric construction token also has no
issuer/collision/exhaustion rule and needlessly replaces the current Arc-backed
`RuntimePlanConstructionIssuer` identity.

Current runtime-plan lowering consumes its builder through `finish()` and
returns a public plan. The return does not define how the compiler can first
obtain coordinates, construct and validate the upper View binding, and then
seal that same unconsumed builder exactly once. A partial plan or reopened
builder would create a second authority.

Finally, `finish_authority_transcript(self, blake3::Hasher)` lets any public
trait implementation supply a noncanonical prefix, and the evolved trait drops
the current live generation/producer/outcome/request validation method. The
correction keeps core's preseeded prefix private, exposes one typed View
completion operation only, and extends rather than replaces live validation.

## Retained design direction

The following results remain useful input to the correction and do not
authorize production publication by themselves:

- a structured task plan has no self or expected digest field;
- executable hashing uses construction coordinates rather than completed plan
  keys, eliminating the return edge in the digest graph;
- expected decoded keys are private assertions checked after recomputation;
- core owns the static row and common seal while the bundle/View product owns
  real View identity and admission;
- ordinary-only plan sealing does not require a View authority; and
- final rows use one global duplicate check before atomic publication.

## Blocking correction

The exact owner and predecessor gaps are assigned to:

- [`Lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1-current-runtime-plan-semantic-owner-and-view-predecessor-reconciliation-correction.md).

Sol max closed every independent correction direction in the associated
[blocked design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1-current-runtime-plan-semantic-owner-and-view-predecessor-reconciliation/README.md).
Its status remains `DESIGN_BLOCKED_ON_ACCEPTED_PREDECESSORS`; its five open
questions are exactly the `.1.2`/`.1.4` output types and transcripts that must
not be guessed.

No production compile, test, Clippy, generated-artifact, AOT, browser, or
platform tier was run because the returned archive is design-only and failed
repository reconciliation before implementation.
