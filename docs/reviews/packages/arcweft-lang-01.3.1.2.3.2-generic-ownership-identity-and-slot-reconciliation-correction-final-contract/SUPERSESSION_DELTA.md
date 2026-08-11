# Narrow supersession delta

## 1. Authority set

| Artifact | SHA-256 | Status in this correction |
|---|---|---|
| Lang-01.3.1.2.3 affine runtime-value owner/capture package | `d053fae201afa104f7db9914aebbc08f2456875d1229f5325f86235d4bc0ea94` | retained except rows below |
| Lang-01.5.1.1.2 final-HIR/View package | `87b7f7bea85bc54254e3a979f0d668026ab75cb1c71955fd7a0f740e4f30c1c6` | context only; no independent View decision changed |
| Lang-01.3.1.2.3.1 affine/View/ABI1 correction | `a52453fd07fdacf10205cbf621077f923ded714b83e4c64b9b69c52a7350ff7f` | retained except undefined activation/identity rows below |
| implemented classifier | Git `b76465c128322be2d5e66398bc6c30794ca0276f` | preserved implementation authority |
| this package | generated archive identity in `FINAL_STATUS.md` | final identity/slot/transaction correction |

Any parent statement not identified in the table below remains normative.

## 2. Superseded rows

| ID | Parent/incomplete result | Final replacement | Retained parent result |
|---|---|---|---|
| DELTA-ID-001 | `ExecutionInstanceId` referenced without exact owner/representation | private `NonZeroU64` in core runtime ID; domain-only monotonic mint; strict codecs | execution-scoped affine evidence |
| DELTA-ID-002 | execution identity could be UUID/content/host/random/local integer | domain ordinal, starts 1, never reused, preserved by restore/replay/restart | no host-created token |
| DELTA-ACT-001 | per-`RuntimeDriver` mutable borrow claimed activation exclusivity | one host-shared `RuntimeExecutionDomain` plus non-Clone reservation/active owner | whole-execution dormant candidate |
| DELTA-ACT-002 | `.3.1` accepts undefined `RuntimeFreshExecution` | exact fresh/source/mode/session/reservation owner and owner-return errors | empty/replacement activation distinction |
| DELTA-ACT-003 | copied images could run in two drivers | domain has ≤1 reservation and ≤1 active execution; second driver rejected | images remain copyable dormant data |
| DELTA-CUR-001 | affine allocator continuation after restore unspecified | persist `next_affine_owner`; require cursor strictly above every recorded owner | preserved owner IDs |
| DELTA-CUR-002 | other occurrence/slot/transaction cursor behavior unspecified | persist all four execution-local cursors and domain next-execution cursor | deterministic replay/save |
| DELTA-REC-001 | record paths by name/layout/authored ordinal unresolved | `RuntimeRecordFieldId(NonZeroU32)` is one-based accepted storage/layout ordinal | current ordered vectors |
| DELTA-REC-002 | duplicate field names could affect traversal | reject duplicates before publishing IDs/traversal | diagnostic names retained |
| DELTA-LOC-001 | runtime locals name-indexed only; no stable slot | execution-wide nonreused `RuntimeLocalSlotId` plus plan declaration ID | existing nested `RuntimeEnv` |
| DELTA-LOC-002 | HIR `LocalId` mapping could reverse dependency | transient sema/runtime-plan map; core stores projected IDs only | typed HIR identity remains compiler authority |
| DELTA-LOC-003 | mutation/suspension restore revision undefined | `RuntimeSlotRevision`, initial 1, checked +1 per commit, persisted | parent mutable binding semantics |
| DELTA-SLOT-001 | placeholder/reduced `RuntimeOwnedSlotId` | exact eight-variant diagnostic enum/tags/order | evidence, not storage |
| DELTA-SLOT-002 | owner enum behavior could live in helpers | inherent tag/execution/render/order/codec methods | no extension trait |
| DELTA-TXN-001 | transaction ID, limits, prepared records were prose-only | exact owners/APIs/errors/limits/traits in `RUST_OWNERS_AND_APIS.md` | staged Copy/Move/Drop |
| DELTA-TXN-002 | prepared Drop could accept value A then commit arbitrary B | Drop prepares against exact reserved slot; commit accepts no value | preflight/stage/commit model |
| DELTA-TXN-003 | Move commit identity check could be impossible without Clone/Eq | exact slot handle+revision+reservation; source remains in slot until permit | live affine values remain non-Clone |
| DELTA-TXN-004 | failure owner return unspecified | prepare returns transaction; mismatch returns non-recommittable aborted owner | failure atomicity |
| DELTA-TXN-005 | point of infallibility unclear | successful `RuntimeCommitPermit` construction; no fallible branch thereafter | staged commit |
| DELTA-PATH-001 | path shapes/order incomplete | exact ten-segment enum, tags, manual order, one visitor | parent value graph |
| DELTA-PATH-002 | iterator path could include consumed prefix or suffix-relative index | current remainder only; absolute original indexes | shipped classifier behavior |
| DELTA-PATH-003 | nominal/authored order ambiguous | anonymous accepted authored order; nominal accepted layout order | existing nominal schema vector |
| DELTA-ERR-001 | first-error selection could depend on iteration | exact prepare/commit ranks then slot/path/owner/step | deterministic diagnostics |
| DELTA-SNAP-001 | live runtime value/binding Serde assumptions | parent closed snapshot carrier plus identity envelope; live carriers not save format | save-schema-2 direction |
| DELTA-SNAP-002 | `RuntimeValueSnapshotV2` declared both Eq and non-Eq | bit wrappers; `PartialEq`, not `Eq`/`Hash` | exact float bits preserved |
| DELTA-SNAP-003 | identity tamper order incomplete | fixed 12-stage validation before reservation/activation | atomic candidate construction |
| DELTA-DIG-001 | execution/slot/owner identity omitted from digest | one domain-separated identity section in existing digest | no second digest |
| DELTA-ORD-001 | G1.2 symbols could land in one non-compiling bulk switch | six compile-clean G1.2 cuts before G1.3/G1.4 | parent interleave and no-handle boundary |

## 3. Explicitly retained rows

This correction does **not** change:

- `RuntimeValueOwnership::{Unrestricted, Affine}`;
- current ownership result for any constructible production value;
- checked unrestricted duplication semantics except exact error/transaction
  plumbing;
- the single opaque affine token and future Stream handle direction;
- capture set, transfer mode, pattern, constant, and closed-payload decisions
  already fixed by the parents;
- View retained/render unrestricted-only admission;
- View handler Move;
- the static-fragment dispatch correction outside the identity rows;
- ABI/opcode/type-tag/section allocation;
- source syntax, ordinary-function/direct-suspension direction;
- Stream lifecycle/policy/replay/publication;
- host payload shape outside the execution identity envelope;
- Proof identity or concurrency schema; or
- any G1.3/G1.4 production work.

## 4. Version reconciliation

The request requires preserving the parent affine/View target of ABI 1 /
codec 8. Current inspected production may report a different internal codec
number due later unrelated commits. This package allocates no number and
therefore neither overwrites production with codec 8 nor changes the parent
target.

At implementation intake, the owning AWBC/save modules must rebase against the
actual current constants. Identity state remains outside AWBC wire in G1.2.
There is no compatibility interval or dual reader.

## 5. Audit finding closure

| Audit finding | Closure |
|---|---|
| activation exclusive only per driver | shared execution domain + reservation/active owners |
| affine cursor undefined after restore | serialized cursor + max-used strict validation |
| prepared Drop may commit unrelated value | exact reserved slot; no value parameter |
| floating snapshot Eq contradiction | bit carriers; PartialEq only |
| parent archive mechanical validation not production proof | `VALIDATION.md` records exact package/production limits |

## 6. Non-supersession test

A review must reject any implementation diff that changes a parent decision not
listed in §2 under the pretext of implementing this correction. Discovery of an
unrelated defect requires a separate typed correction, not an opportunistic
helper/compatibility layer here.
