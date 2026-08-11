# Requirements traceability

## 1. Contract status and authority

This archive is self-contained for the unimplemented portion of proof-concurrency cut 01.1.1. Latest repository policy and latest `main` evidence were applied. No earlier design archive, unpublished patch, or conversation history is required. The request's design-only prohibition is honored: this archive contains no production checkout or patch.

| Request area | Normative decision location | Direct evidence/test location |
|---|---|---|
| goal, authority, supersession, task mode | `README.md`, `DESIGN.md`, `FINAL_STATUS.md` | `REPOSITORY_EVIDENCE.md`, archive manifest checks |
| latest main, Git/JJ identity, current starting point | `REPOSITORY_EVIDENCE.md` | implementation identity commands in `VERIFICATION_PLAN.md` |
| current `AGENTS.md` policy | `DESIGN.md`, `MIGRATION_AND_DELETION.md`, `STRUCTURE_PLAN.md` | compile-fail, metadata graph, structural audit gates |
| no production changes in returned package | `FINAL_STATUS.md`, `README.md` | archive-member inspection; no checkout/patch member |

## 2. Required decision sections 1-11

| Request section | Closed decisions | Normative files | Tests/gates |
|---:|---|---|---|
| 1. Grammar-level lossless tree and identity-bearing inventory | exhaustive grammar families; ID-bearing versus ID-less nodes; tokens receive no IDs; delimiter/missing/recovery identity; one event/build pipeline; layout sugar representation; grammar-parent reconciliation; repeated/moved/copied/missing behavior; atomic migration | `LOSSLESS_TYPED_IDENTITY.md` sections 1-4, 8, 10-12; `API_AND_DIAGNOSTICS.md` | `TEST_MATRIX.md` section 2; syntax focused commands |
| 2. Typed AST attachment and snapshot ownership | immutable `AstNode<K>` handles owning snapshot `Arc`; database/lineage/snapshot/node IDs; constructor visibility; equality; range/span updates; direct Rowan round trips; wrong database/lineage/snapshot/generation errors; bound/unbound fragment type separation; exact end-to-end call sequence; parse/attachment rollback | `LOSSLESS_TYPED_IDENTITY.md` sections 5-11; `API_AND_DIAGNOSTICS.md` sections 1-4 | syntax identity and compile-fail suites in `TEST_MATRIX.md`; sections 3 and 7 of `VERIFICATION_PLAN.md` |
| 3. Complete predicate/proof grammar | visibility/docs/attributes, ordinary names, generics, exactly one fixed parameter group, typed patterns/types, where, return, requires/ensures, expression/block body; implicit Bool predicate; Unit/non-Unit proof rules; logical lines; ordinary namespace/import behavior; recursion rejection; exact limits; recovery and removed-form ordinary recovery | `PREDICATE_PROOF_GRAMMAR.md`; diagnostics in `API_AND_DIAGNOSTICS.md` | `TEST_MATRIX.md` section 3 |
| 4. Exact `ProofBlock` and typed body | exact Rust types/fields/accessors/constructor ownership; expression/block/open/close/stmt/tail identity; dedicated validated statement boundaries; pure let/proof call/assertion use existing typed authorities; predicate/proof context restrictions; implicit Unit synthetic role; poison/executability; balanced/malformed ranges; counting point | `PROOF_BLOCK.md`; `PREDICATE_PROOF_GRAMMAR.md`; `API_AND_DIAGNOSTICS.md` | exact range/shape/context/limit rows in `TEST_MATRIX.md` section 3 |
| 5. `HirDatabase`, immutable snapshots, arenas | owning modules; database/module/revision/slot state; `LoweringRequest`; immutable module metadata/status; private typed paged arenas; source/synthetic slot metadata; live intervals; typed resolution; no-op/stale/cross-database/recovered/cache behavior; all limits/exhaustion | `HIR_DATABASE_AND_ARENAS.md`; consolidated signatures in `API_AND_DIAGNOSTICS.md` | `TEST_MATRIX.md` section 4 and HIR compile-fail tests |
| 6. Scopes, locals, captures, direct typed lowering | exact scope/local/capture records; allocation keys; pre-binding initializer; destructuring order; irrefutability/duplicates/underscore/poison; shadow generations; mutable binding versus reference; closures and capture ordering; control scopes; result visibility; direct typed lowering without clones/reparse | `SCOPES_LOCALS_CAPTURES.md`; HIR records in `HIR_DATABASE_AND_ARENAS.md` | scope/local/capture rows in `TEST_MATRIX.md` section 4 |
| 7. Atomic HIR transaction | private staging owner; arena/allocation/live interval/generation/capture/diagnostic/source/cache state; exact phases; commit-only mutation; recoverable commit versus fatal rollback; diagnostic ordering/dedup; failure construction owner | `HIR_DATABASE_AND_ARENAS.md` sections 13-16; `API_AND_DIAGNOSTICS.md` | HIR limit/exhaustion and stale/foreign/invariant rows in `TEST_MATRIX.md` section 4 |
| 8. Module-preserving project and symbols | exact project/module/view types; checked package/path/source construction; no clone/rebase/flatten; per-module exported/style aggregation; unified symbol table registration and callable owner extension; imports/aliases/duplicates/invalidation; session-only proof artifact; full caller migration | `PROJECT_AND_SYMBOLS.md`; `MIGRATION_AND_DELETION.md` | `TEST_MATRIX.md` section 5 and project/HIR compile-fail suites |
| 9. Runtime assertion-fault identity | exact session fault/site/index/mode/span; separate presentation; persisted guard and artifact fingerprint; runtime-plan inventory/capability; Debug omission; Prove impossibility; serialized versus non-serialized boundary; load/reassociation contract; stable CLI/LSP/Agent projection; core dependency boundary | `RUNTIME_ASSERTION_FAULT.md`; `API_AND_DIAGNOSTICS.md` sections 7, 11 | `TEST_MATRIX.md` section 6; codec, metadata, runtime-plan commands |
| 10. Migration and deletion inventory | current files/symbols/callers; grammar/typed/HIR/project/runtime/tooling migration; deletion of provisional proof/trusted/raw body/authored IDs/old assertions/detached lowering/linked HIR; no compatibility aliases or dual readers | `MIGRATION_AND_DELETION.md`; safe order in `IMPLEMENTATION_PLAN.md` | compile-fail and direct behavior rows in `TEST_MATRIX.md` section 7 |
| 11. Structure and decomposition | exact available baseline metrics; mandatory responsibility modules; no hotspot append; facade/module size gates; generated status; dependency direction/fan evidence; exact implementation audit outputs | `STRUCTURE_PLAN.md`; baseline in `REPOSITORY_EVIDENCE.md` | canonical structural audit and metadata commands in `VERIFICATION_PLAN.md` |

## 3. Direct test matrix groups

| Required group | Coverage |
|---|---|
| lossless and typed attachment | `TEST_MATRIX.md` section 2 covers same-line distinct IDs, trivia, changed/reorder/move/copy/recovery/missing behavior, round trip, stale/wrong/foreign database errors, atomic parse/attachment/identity failures, and compile-time fragment separation |
| predicate/proof/`ProofBlock` | section 3 covers complete grammar, exact shape/ranges, pure lets, proof calls, assertion modes/context, malformed recovery, all inclusive limits, and ordinary recovery for removed forms |
| HIR arenas and lowering | section 4 covers source-kind mapping, revision behavior, liveness errors, scopes/locals/captures, all HIR limits and exhaustion, typed-child authority, and recovered cache exclusion |
| project and symbols | section 5 covers preserved module IDs/order, checked construction, one table, collisions/visibility/imports/aliases/invalidation, proof artifacts, and absence of flattened/public provisional APIs |
| runtime assertion fault | section 6 covers Check/Debug identity, condition indices, Prove absence, release omission, typed guard/fingerprint codecs, all persistence owners, dependency graph, presentation, and fresh-session load behavior |
| deletion/tooling/recovery | section 7 covers parser/formatter/LSP/CLI/compiler/sema/verifier/runtime-plan/project/cache consumers, following-declaration recovery, public API absence, and the prohibition on source-text audits |

## 4. Fixed substrate preservation

| Fixed authority/invariant | Preservation decision |
|---|---|
| exact `SourceDocumentIdentity` versus incremental `SourceSnapshotId` | retained as separate authorities; syntax adds database/lineage identity without changing source provenance |
| `SyntaxDatabase` atomicity, no-op, generations, never-reused IDs | retained and generalized to grammar nodes; transaction stages all new state |
| accepted reconciliation behavior | retained by exact-subtree, same-parent sequence, distance/old-ID tie policy over semantic grammar parents |
| `BorrowKind`, reference/prefix semantics | carried into typed HIR by existing enum values; no redesign |
| typed `AssertionMode` semantics | existing enum remains source/HIR authority; runtime conversion is an inherent checked narrowing |
| exact `SourceSpan` construction | all spans constructed through exact retained `SourceDocument` revisions |
| `ProjectSymbolTable` and Character authority | extended directly; no second callable/proof/source authority |
| Sans-I/O and dependency direction | frozen in `DESIGN.md`, runtime/core boundary, and metadata validator |

## 5. Constraints and non-goals

| Constraint/non-goal | Closure |
|---|---|
| source/syntax/data formats remain Sans-I/O | no I/O API introduced; verification checks dependency graph |
| syntax remains parser-only | HIR/sema/runtime work stays in downstream crates |
| core remains runtime/data-only | only fixed-byte guard/fingerprint and serialized runtime assertion data live in core |
| no public raw IDs or session-ID Serde | constructor/Serde compile-fail tests |
| no flattened HIR/compatibility/deprecated/dual readers | exact deletion gates and compile-fail tests |
| no local extension traits/string tags/source gates | inherent enum implementations and typed newtypes only |
| no removed-syntax production recognizers/diagnostics | ordinary current recovery tests; temporary recognizer deletion gate |
| no proof discharge/solver/Copy-Move/borrow dataflow/scheduling/checkpoint/AWBC redesign | explicitly excluded in `DESIGN.md` and `RUNTIME_ASSERTION_FAULT.md` |
| exact runtime fault identity is in scope | fully specified and tested without implementing general runtime evaluation |

## 6. Required implementation order

The exact order is frozen in `IMPLEMENTATION_PLAN.md`:

1. baseline/freeze APIs, limits, diagnostics, HIR arenas, runtime boundary;
2. private grammar event/build pipeline;
3. private reconciliation and typed attachment;
4. atomic syntax public switch and fragment migration;
5. final predicate/proof grammar and `ProofBlock`;
6. private HIR database/arenas/scopes/locals/captures;
7. atomic HIR/project/symbol/sema/verifier public switch;
8. runtime assertion identity/persisted-boundary switch;
9. compiler/runtime-plan/CLI/LSP/tooling/formatter/cache/docs migration;
10. deletion, full direct tests, workspace validation, and structural audit.

The safe states explicitly prohibit mixed line/grammar HIR IDs, two symbol tables, serialized session IDs, or simultaneous public linked/arena HIR contracts.

## 7. Verification requirements

| Required verification | Exact command location |
|---|---|
| focused source/syntax/HIR/sema/verifier/runtime-plan/compiler/runtime host/CLI/LSP/tooling/codec tests | `VERIFICATION_PLAN.md` sections 3-6 |
| compile-fail suites | section 7 |
| format | section 9: `cargo fmt --all -- --check` |
| workspace check | section 9: exact `CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features` |
| Clippy | section 9: exact `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| checked-in fast cut point | section 10: `CARGO_INCREMENTAL=0 just verify` |
| conditional Tier 2 | section 10: `CARGO_INCREMENTAL=0 just verify-full` only on named risk-owner touch |
| dependency evidence | section 8: Cargo metadata, typed validator, cargo trees |
| diff/whitespace/conflicts | section 11 |
| structural audit | section 12: exact nightly script command and report requirements |
| no unrun claim | sections 1 and 13; `REPOSITORY_EVIDENCE.md` section 9 |

## 8. Archive and sidecar requirements

| Output rule | Satisfaction |
|---|---|
| exact archive name | produced exactly as required |
| exactly 20 sorted members | enforced by package validation script and ZIP central-directory inspection |
| manifest lists every member lexically | `MANIFEST.txt` generated from exact member set |
| lowercase SHA-256 | generated with Python `hashlib.sha256` and independently rechecked with `sha256sum` |
| manifest self-entry | 64 lowercase zero characters for `MANIFEST.txt`, documented in `README.md` |
| `OPEN_QUESTIONS.md` exactly `none\n` | byte-level package check |
| summary/status/hash sidecars | created outside ZIP; status values cross-checked with `FINAL_STATUS.md` |
| ZIP/hash only when ready | status is ready with zero result-changing decisions |
| no checkout, `.git`, target, cache, patch, build output, credentials, secrets, or fabricated logs | exact member allowlist and ZIP inspection |

## 9. Decision-completeness statement

Every implementation-selectable boundary named in the request has one owner, one API shape, one constructor/visibility policy, one range/recovery rule, one atomic failure rule, one migration/deletion point, and direct tests. `OPEN_QUESTIONS.md` contains no open item. Implementation may choose private algorithmic details only where they are observationally irrelevant and remain inside the exact contracts above; it may not choose a different public authority or compatibility path.
