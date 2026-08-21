# Lang-01.5.1.1.2.1.1.1.1 checked Match / Need identity return intake

Date: 2026-08-22
Inspected Git commit: `cbf0acedb98de260d8ecaab70a39933c39f30708`
Working tree before intake: clean; `main` matched `origin/main`

## Intake result

- Archive safety and integrity: `PASS`
- Internal package validator: `PASS`
- Repository reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: `BLOCKED_FOR_THE_RETURNED_COMBINED_CONTRACT`
- Independently usable accepted subset: checked Match usefulness analysis after
  the guard and View-admission corrections recorded below
- Open questions claimed by the package: none
- Production source, tests, fixtures, or generated artifacts changed by the
  package: none

The return closes several predecessor gaps, but its nonnumeric runtime identity
and admission model is not implementable as written. The failure is independent
of the package's obsolete AWBC allocation. The maintained semantic-range
allocation remains authoritative and will be projected locally; it is not
reopened by the correction request created from this intake.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip`
- byte length: 108,309
- SHA-256:
  `DDD097E8057A8D45018528431790C20A2DE665CDE40F0329B82CB0366CF95D32`

The unchanged byte authority is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip).
Its 48-file byte-identical frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/README.md).

## Performed and passed

- Verified one exact top-level wrapper, 48 members, 260,017 uncompressed
  bytes, no absolute/drive/parent-traversal path, no duplicate entry, and no
  case-fold collision.
- Verified the retained ZIP is byte-identical to the external attachment.
- Verified all 48 extracted files and all 45 `MANIFEST.json` payload rows by
  independent length and SHA-256 calculation.
- Verified `MANIFEST.json` SHA-256
  `09D5A2EA8401EABD95AD95B7C14DCA163E7D4802A87C887F2B8D3BEEDB93E86D`
  equals `MANIFEST.sha256`.
- Inspected the standard-library-only validator before execution. It performs
  read-only package checks plus temporary ZIP extraction and contains no
  subprocess, network, repository write, or production mutation path.
- Ran the validator against both the extracted directory and retained ZIP with
  `uv run --no-project`; both reported `PASS`.
- Enumerated all 60 retained `docs/reviews/**/*.zip` archives at the reviewable
  cut. The sorted relative-path, byte-length, SHA-256 LF transcript hashes to
  `8A7C4AD24B21F223DE5DFB0CF67492213A286F871D303932FA93228E351C847C`.
- Confirmed the package baseline `c49099fb154d9e3dbb587e1bcd7ee243214da0c4`
  and current HEAD differ only by documentation/package intake commits for the
  production paths under review.
- Confirmed the returned Maranget-style coverage owner, complete pattern-family
  matrix, dynamic-guard non-contribution, hard non-exhaustiveness, retained
  unreachable evidence, and bounded-work direction are implementable after the
  exact Boolean-literal guard correction below.
- Confirmed `RuntimePlanSemanticFactInput` and functional
  `awbc::vm::step`/`step_with_host` are current constructible owners.
- Obtained an independent Sol-max design audit of the nonnumeric contract.

## Superseded package rows that do not cause this failure

The package was produced from the pre-reorder request copy, SHA-256
`8BF22DBEE57A94EE178E25D0004BE7A18694A8B801EF79189DA3F9E1A3741299`.
The maintained request is now SHA-256
`F6DC1C21AC4A80755DFEB34B26138508834FF9700CCF2A8ED60E9F049038AE40`
because the user-directed Sol-max allocation reorder was recorded after the
package baseline.

Accordingly, the package's opcode bytes, function-kind tags, flag positions,
tombstones, numeric goldens, and numeric compile sequence are frozen evidence
only. The maintained family-range allocation in
[`executable-runtime-core.md`](../02-runtime/executable-runtime-core.md) and the
current correction request remain the sole implementation authority. No old
reader, translation table, tombstone, or compatibility alias is authorized.

## Failed repository reconciliation

### AlwaysStart conflates distinct terminal cells

The return requires equal producer inputs under `AlwaysStart` to retain one
`NeedId`, while every launch receives a distinct `TaskKey`/`TaskId` and a
different terminal result for that same `NeedId` is a correlation conflict.
Distinct AlwaysStart I/O launches may legitimately complete differently. The
second result would therefore be rejected even though it belongs to another
launch. The proposed `TaskKey` also includes the launch ordinal and `TaskId`
hashes the same ordinal again, conflating coalescing and launch identity.

### View identity uses nonexistent and cyclic owners

Current production owns `ViewProgramId` and
`AcceptedViewProgramRevision([u8; 32])` in
`crates/arcweft-view/src/view/identity.rs`; it does not own the package's
`ViewProgramSemanticDigest` plus canonical-u32 revision pair. The accepted
revision is derived from the View semantic transcript. Hashing that revision
inside a Match digest which is then retained by the View program is cyclic.
Hashing revision into View `NeedId` also makes the package's claimed
semantic-equality generation rebind impossible without an unspecified identity
translation.

### Generic Match and retained View admission are mixed

The return makes ownership/persistence admission part of generic
`CheckedMatch::try_from_hir`. This would reject ordinary language Match over an
affine, Stream, callable, or other non-persistable value even when the value is
legally moved or destructured and never retained by View. Coverage belongs to
the generic Match fact; retained binding ownership belongs to a separate
checked View admission product. A View admission failure must not erase the
generic Match fact.

### Ownership inputs and rows are not constructible

- `AcceptedNominalSemantics::Opaque` receives value-class/persistence fields,
  but current `AcceptedNominalInventoryInput` carries only the producer. The
  registrar cannot construct the proposed semantic row without guessing.
- `TypeKind::AgentResource` and `AgentResourceBody` carry no resource-registry
  identity, so the proposed exact `ResourceTypeRegistry` lookup has no key.
- The proposed `RuntimeNeedHandle` owns a boxed argument vector and admits
  `SnapshotClone` arguments; its type-level `Need<T>` disposition cannot be
  `Copy`.
- the current `Ref` carrier is String-backed and is not a `Copy` value;
  `ViewValue` has no unconditional runtime snapshot authority.
- producer admission incorrectly manufactures producer contract identity from
  ownership evidence instead of leaving identity with the function/site/plan
  owner.

### Digest and event APIs remain incomplete

The return lists domains but does not define the ordered inputs of
`NeedProducerContractDigest`, the exact ordered-argument/source digest, or a
complete event-to-journal correlation schema. It redeclares
`RuntimeValueDigest` despite the existing
`arcweft_core::entry::RuntimeValueDigest` and existing canonical
`RuntimeValue::try_canonical_bytes`/`try_digest` authority. It also requires
additional `TaskEvent` fields only in prose while current `TaskEvent` contains
`logical_epoch`, `task_id`, `sequence`, and `kind`.

`AwbcTaskProducer.plan_digest` is stored inside the same plan from which it is
fully derived. This duplicates authority; the plan should compute its semantic
digest and only external binding/snapshot owners should retain an expected
digest.

### Guard owner and compile-clean sequence are inaccurate

Current final analysis has Boolean literal resolution but no general checked
constant-fold result. Only an exact checked Boolean literal may currently be
classified ConstantTrue/ConstantFalse; all other guards are Dynamic. A
ConstantFalse guard owns `FalseGuard` precedence independent of pattern
coverage.

The package deletes String task identities and direct Await in Cut 3, adds the
typed Need carrier in Cut 5, and postpones journal/save/replay migration until
Cut 10. With no compatibility fallback, the intermediate cuts cannot remain
compile-clean and executable. The fixed identity, typed carrier, event,
journal, Await/AwaitMany, snapshot/restore, and adapter switch must be one
protected publication cut or remain private staging until that cut.

## Sol-max root reconciliation selected for the correction

The follow-up audit selected one mandatory architecture rather than leaving
alternatives to the next implementer:

- one `NeedProducerInstanceKey` commits producer family, contract, plan, site,
  payload type, and the existing canonical runtime-value argument digest;
- JoinSameKey uses launch ordinal zero and derives one stable
  NeedId/TaskKey/TaskId, while AlwaysStart allocates a journal-owned ordinal
  from one and derives a distinct NeedId/TaskId for every launch;
- TaskKey excludes launch ordinal, and TaskId includes it exactly once;
- reusable pre-launch `RuntimeNeedHandle` is JoinSameKey-only; AlwaysStart
  returns a concrete handle from its accepted launch;
- `GenerationId` moves from runtime-driver into `arcweft-core::task`, and
  `TaskHost::ensure_task` derives correlation rather than accepting caller-made
  NeedId/TaskKey/TaskId/ordinal fields;
- the existing `RuntimeValueDigest` and canonical runtime-value visitor remain
  the sole value grammar; the visitor becomes sink-parametric for allocation-
  free hashing, and empty arguments use canonical `RuntimeValue::Tuple([])`;
- fixed Need/Task/producer-instance ID zero is a typed error with no rehash,
  while semantic digest types retain their own existing full-output policy;
- generic `CheckedMatchSemanticDigest` contains language Match meaning only;
  `CheckedViewMatchAdmissionDigest` separately contains retained outputs,
  captures, exact ownership evidence, and producer admission;
- `ViewProgramId` and stable site form the product coordinate, while current
  `AcceptedViewProgramRevision([u8; 32])` is used only for catalog/bundle/
  replacement validation and never enters Match, admission, or Need identity;
- opaque value-class/persistence evidence is mandatory from
  `AcceptedNominalInventoryInput` through registrar and accepted catalog, with
  no default or side table; and
- the public task/Need carrier, events, journal, snapshots, replay,
  replacement, adapters, and String-route deletion form one indivisible atomic
  switch after private identity preparation.

These choices are recorded as exact required schemas and transcripts in the
new request. Numeric AWBC allocation remains external and final.

## Blocking correction and allowed independent work

The returned combined contract is blocked by the new nonnumeric correction
request:

- [`Lang-01.5.1.1.2.1.1.1.1.1`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction.md).

That request does not reopen AWBC numeric allocation. It selects distinct
producer, terminal-cell, coalescing, and launch identities; current View
revision roles; generic Match versus checked View admission separation; the
opaque evidence publication path; exact digest/event schemas; and an atomic
identity/carrier/persistence cut.

The generic checked-Match coverage owner is independently implementable when it
uses only existing Boolean-literal constant evidence and does not run View
ownership admission. Typed Need identity/carrier publication, View retained
admission, and persistent consumer migration remain blocked pending the
correction.

No Rust, Cargo manifest, production fixture, generated artifact, Clippy, AOT,
platform, browser, or runtime test was changed or run for this intake cut.
