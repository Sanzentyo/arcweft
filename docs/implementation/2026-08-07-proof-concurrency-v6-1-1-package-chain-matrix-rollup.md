# Proof-concurrency v6.1.1 package-chain matrix rollup

- Date: 2026-08-07
- Inspected Git commit:
  `80331c81e338d20e968a10947d5e848c39610384`
- Working tree: dirty Proof public-switch integration on
  `codex/proof-public-switch`
- Status: `COMPLETE_FOR_IMPLEMENTABLE_SCOPE`
- Base-package row ledger:
  [2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note is the precedence-aware rollup for the base Proof-concurrency
v6.1.1 matrix and its accepted or repository-adjudicated correction chain.
The base ledger remains the row-by-row execution authority for the base ZIP;
this note adds the correction matrices and records which local rows remain
effective, superseded, normalized, or design-blocked.

It is not valid to add every archive's row count together. Later packages
restate and replace earlier local rows, local IDs such as `T-MIGRATION-01`
collide across package namespaces, and the Call and ordinary-Flow archives are
usable only through their repository adjudications. The stable rollup key is:

```text
(authority_key, local_row_id)
```

## Ledger columns

| Column | Meaning |
|---|---|
| `authority_key` | Stable package or repository-adjudication namespace. It is required even when a local row ID appears unique. |
| `local_row_id` | Exact row ID or a closed local row range from the named authority. A range must be expanded before final row-level completion. |
| `effective_contract_state` | `EFFECTIVE`, `EFFECTIVE_WITH_NORMALIZATION`, `SUPERSEDED`, `NOT_APPLICABLE_WITH_EVIDENCE`, `HISTORICAL_REJECTED`, or `DESIGN_BLOCKED`. |
| `supersedes` | Earlier qualified rows or claims replaced by this row. Empty means the row is additive or retained. |
| `normalized_expectation` | The exact repository-selected behavior after applying precedence. It must not be inferred from a filename or ZIP self-status. |
| `test_owner_candidate` | Current typed behavior owner that may supply exact or explicitly equivalent execution evidence. Static presence is not PASS evidence. |
| `execution_state` | `IMPLEMENTED_NOT_RUN` until the mapped behavior runs successfully on the coherent public-switch copy; `PASS` after exact normalized behavior executes successfully; `BLOCKED` only for the named design-blocked rows. Non-effective historical rows use `—`. |
| `evidence` | Exact command, selected test count, result, date, and final Git revision. Candidate filenames or source scans are not execution evidence. |

When a row runs, append its exact evidence without rewriting its contract
state. A later accepted correction changes `effective_contract_state` and
`supersedes`; an ordinary test run changes only `execution_state` and
`evidence`.

## Targeted authority precedence

Precedence is boundary-specific. A later authority replaces only its named
area and does not reopen unrelated accepted owners.

| Priority within an overlapping boundary | `authority_key` | Repository adjudication | Effective boundary |
|---:|---|---|---|
| 1 | `FLOW_ADJ_2_1_1_1` | repository adjudication releases implementation; ZIP standalone READY claim rejected | ordinary Flow, shared Thread/Flow items, scope/source/limits, and Choice expression |
| 2 | `SELECT_CENTRAL_4_1_1_1_1_1_1` | accepted with evidence normalization | E13 Select producer, payload, source, accounting, rollback, and deletion |
| 2 | `CALL_ADJ_4_1_1_1_1_2_1` | returned ZIP rejected; intake's repository decisions release implementation | E12/C01-C03 Call, cursor, resolver, accounting, source, limits, and deletion |
| 3 | `TAIL_4_1_1_1_1` | accepted correction | typed synthetic roles, tail owners, production generators, liveness, and fingerprints, except the two blocked roles below |
| 4 | `SOURCE_4_1_1` | accepted correction | sole source query, pathless variants, Duration identity, checker overflow, typed synthetic owner, and HIR limits |
| 5 | `LEAF_4_1` | retained where uncontradicted | final leaf expression/pattern/component payloads and same-arena ownership |
| 6 | `BASE_6_1_1` | accepted | qualified syntax/HIR/project, transactions, runtime assertion, codec boundaries, and base matrix |

Repository-wide instructions, maintained current design, and the explicit
repository adjudications recorded below take precedence over a conflicting ZIP
row. Current production code is migration and consumer evidence; it does not
silently redesign an accepted row.

## Matrix cardinality and local ID namespaces

| `authority_key` | Matrix cardinality | Local ID namespace |
|---|---:|---|
| `BASE_6_1_1` | 157 owner-qualified occurrences; 153 unique acceptance identities | exact test names and fixture paths in `TEST_MATRIX.md`; four syntax/HIR limit names intentionally occur twice |
| `LEAF_4_1` | 82 lowering rows; 99 test rows | `E01..E35`, `P01..P12`, `C01..C35`, `T-E01..T-E35`, `T-P01..T-P12`, `T-C01..T-C35`, plus 17 named cross-row tests |
| `SOURCE_4_1_1` | 82 corrected lowering rows; 106 top-level tests; 164 named subtests | the same `E`, `P`, and `C` ranges plus typed query/rollback/path/Duration/limit/migration rows |
| `SYNTHETIC_REJECTED_4_1_1_1` | 21 role rows; 56 tests | historical only; replaced by `TAIL_4_1_1_1_1` |
| `TAIL_4_1_1_1_1` | 21 role rows; 11 tail-producer rows; 9 affected-lowering rows; 6 generator families; 88 tests | `T-OWNER-*`, `T-ROLE-*`, `T-TAIL-*`, `T-GEN-*`, `T-FP-*`, liveness, limit, compile, and migration rows |
| `SELECT_CENTRAL_4_1_1_1_1_1_1` | 88 executable test rows plus a three-suite summary | `T-E13-001..034`, `T-Q-13-001..024`, `T-RB-13-001..030` |
| `CALL_ADJ_4_1_1_1_1_2_1` | 99 returned test rows: 95 active after adjudication and 4 non-applicable | 33 `T-E12-*`, 10 `T-Q-12-*`, 8 `T-RB-12-*`, 10 `T-RES-12-*`, 10 `T-LIM-12-*`, 8 `T-API-12-*`, 2 `T-DEL-12-*`, and 18 cursor rows |
| `FLOW_ADJ_2_1_1_1` | 148 normative matrix IDs; 306 tests | ID 10, signature 26, contract 46, body 9, item 48, category 8, scope 13, source 18, diagnostic 6, projection 10, validation 14, limit 57, transaction 12, compile-fail 14, migration 12, mode 3 |

The 99-row leaf matrix is not added to the 106-row corrected source matrix as
if both were independent requirements. The source correction is a standalone
complete matrix and replaces the contradictory parent expectations while
retaining uncontradicted leaf behavior.

## Effective row-group rollup

This is the initial expandable rollup. `IMPLEMENTED_NOT_RUN` means that a
typed implementation and plausible test owner exist, but this rollup has not
yet established exact row coverage by executing the behavior on the final
coherent copy. It does not erase more precise base-row results already
recorded in the linked base ledger.

| `authority_key` | `local_row_id` | `effective_contract_state` | `supersedes` | `normalized_expectation` | `test_owner_candidate` | `execution_state` | `evidence` |
|---|---|---|---|---|---|---|---|
| `BASE_6_1_1` | all 157 owner-qualified occurrences | `EFFECTIVE` where not replaced below | — | Use the exact base acceptance identity and owner; keep the four syntax/HIR limit occurrences distinct. | Owners mapped row by row in the base ledger. | `PASS` | The base ledger has zero effective `MISSING`, `PARTIAL`, or unexecuted rows after the 2026-08-09 coherent-copy run; the two external-design role rows are excluded below. |
| `LEAF_4_1` | `E01..E35`, `P01..P12`, `C01..C35` and 99 tests | `EFFECTIVE` where not replaced below | conflicting base final-leaf sketches | One qualified expression arena, known-family poison, typed children, and deletion-driven migration. | `arcweft-lang-syntax` attachment; `arcweft-lang-hir::final_lowering`; source index; final sema consumers. | `PASS` | Final syntax 690/690, HIR 841/841 executed, and sema 163/163 library suites passed on 2026-08-09. |
| `SOURCE_4_1_1` | complete corrected 82-row lowering matrix, 106 top-level tests, 164 subtests | `EFFECTIVE` | leaf source-query, pathless-pattern, Duration, overflow, raw synthetic-owner, limit, and traceability rows | One `HirSourceQuery` over typed Expr/Pattern/Type owners; pathless variants stay typed; checker owns float/Duration overflow; one HIR limit authority. | HIR source-index tests; final type/pattern/expression lowering; literal/path projection; final sema. | `PASS` | The final HIR 841-test run exercised the source-index and complete final-lowering owners; all eight ignored production limits passed separately. |
| `SYNTHETIC_REJECTED_4_1_1_1` | all 56 tests | `HISTORICAL_REJECTED` | — | No direct implementation credit. Non-conflicting transcript input survives only through the accepted tail correction. | — | — | Rejection intake is historical evidence only. |
| `TAIL_4_1_1_1_1` | non-blocked portions of 21 roles, 11 tail producers, 9 affected rows, and 88 tests | `EFFECTIVE` | rejected Expr-only tail owners, identity-only generator evidence, and obsolete liveness wording | `ImplicitUnitTail` and `MissingRequiredTail` use reserved Expr or Scope owners selected by producer; production generators prove order and rollback. | HIR identity tests; final expression/item/pattern/capture/candidate lowering; arena and slot tests. | `PASS` | The final HIR 841-test run executed every non-blocked identity, generator, liveness, and rollback owner. The two unresolved role families remain split below and receive no credit. |
| `SELECT_CENTRAL_4_1_1_1_1_1_1` | `T-E13-001..034`, `T-Q-13-001..024`, `T-RB-13-001..030` | `EFFECTIVE_WITH_NORMALIZATION` | leaf/source E13 plus both rejected Select returns | One central attached Select projection; postfix `?.` is Try followed by Select; final member is `Name | Missing`; one owner-keyed root diagnostic and independent accounting. | syntax expression parser/attachment; HIR `expression_lowering/tests/select.rs`, `select_limits.rs`, and expression source manifest. | `PASS` | Final syntax/HIR suites passed and every Select Tier-2 exact/one-over production boundary passed individually on the coherent copy. |
| `SELECT_CENTRAL_4_1_1_1_1_1_1` | `T-Q-13-016` | `NOT_APPLICABLE_WITH_EVIDENCE` | package spelling `SelectedMember[1]` | `SelectedMember` is fixed and non-ordinal. Validation-order behavior belongs to a real ordinal-bearing `PathSegment` query. | HIR typed source-query precedence tests. | — | No Select ordinal wrapper may be added to make the package fixture constructible. |
| `SELECT_CENTRAL_4_1_1_1_1_1_1` | `T-Q-13-020` | `EFFECTIVE_WITH_NORMALIZATION` | package alternative public query after rollback | A failed transaction exports no public owner; assert no committed source/diagnostic row and deterministic retry. | HIR Select rollback/retry and source-index tests. | `PASS` | Exact normalized rollback/retry behavior passed within the final HIR 841-test run. |
| `CALL_ADJ_4_1_1_1_1_2_1` | 95 active returned tests | `EFFECTIVE_WITH_NORMALIZATION` | leaf/source E12 and C01-C03, detached Call readers, and old Capacity success path | Only a recognized Call family lowers as Call; syntax owns attached vocabulary; callback-block calls remain in the central Call owner; all checked calls use the shared resolver. | syntax parser/attachment call tests; HIR `expression_lowering/tests/call.rs`; expression manifest; final sema resolver; LSP signature projection. | `PASS` | Syntax 690/690, HIR 841/841, sema 163/163, and current LSP signature/cache/state/request/position owner sets all passed on 2026-08-09. |
| `CALL_ADJ_4_1_1_1_1_2_1` | `T-E12-002`, `T-E12-008`, `T-E12-011`, `T-E12-012` | `NOT_APPLICABLE_WITH_EVIDENCE` | unreachable returned Call recovery rows | `(x)` is grouping; `::member(x)` lacks a receiver; missing separator and `..` use ordinary current-grammar behavior. No impossible Call variant is retained. | syntax parser classification tests. | — | Parser behavior and absence of a typed Call owner are the required evidence. |
| `CALL_ADJ_4_1_1_1_1_2_1` | `A-011`, `T-RB-12-004`, `T-LIM-12-008` | `EFFECTIVE_WITH_NORMALIZATION` | returned zero-invocation claim for candidate one-over | Candidate 257 enters the shared resolver exactly once, fails the candidate preflight before any probe, and publishes no semantic report, Call facts, result, physical trace, or retained accounting carrier. Direct failure accounting is logical 1 / resolver 1 / probes 0 / replay 0 / publications 0. | `arcweft-lang-sema::final_analysis::tests::t_lim_12_008_and_t_rb_12_004_candidate_one_over_rolls_back_publication` | `PASS` | `cargo test -p arcweft-lang-sema --lib final_analysis::tests:: -- --nocapture` passed 62/62, including the exact 257 rollback owner, on 2026-08-08 at Git HEAD `52b8c917632358d2360e0bb2ea5c32ecc7ca562b` with the public-switch working copy dirty. |
| `CALL_ADJ_4_1_1_1_1_2_1` | `A-012`, `T-LIM-12-009` | `EFFECTIVE_WITH_NORMALIZATION` | returned combined probe/witness count | Complete sema facts retain both considered candidates and perform two probes plus one selected replay for one argument. The verifier then retains both canonical witnesses with `considered_count = 2` and `omitted_count = 0`; the two-witness bound is not a resolver limit. | sema `multi_candidate_winner_replays_but_singleton_does_not`; verifier `t_lim_12_009_two_candidates_retain_two_witnesses_without_omission` | `PASS` | The sema command above passed 62/62 and `cargo test -p arcweft-verify --lib call_witness::tests:: -- --nocapture` passed 4/4 on 2026-08-08 at the same Git HEAD/dirty working copy. |
| `CALL_ADJ_4_1_1_1_1_2_1` | `A-013`, `T-LIM-12-010` | `EFFECTIVE_WITH_NORMALIZATION` | returned combined probe/witness count | Complete sema facts retain all three considered candidates and perform three probes plus one selected replay for one argument. The verifier retains the primary and first distinct witness with `considered_count = 3` and `omitted_count = 1`; semantic facts are not truncated. | sema `call_adj_a_013_three_candidate_semantic_facts_remain_complete`; verifier `t_lim_12_010_three_candidates_retain_two_witnesses_and_one_omission` | `PASS` | The same sema 62/62 and verifier 4/4 focused runs passed on 2026-08-08 at Git HEAD `52b8c917632358d2360e0bb2ea5c32ecc7ca562b` with the public-switch working copy dirty. |
| `CALL_ADJ_4_1_1_1_1_2_1` | `A-006`, `T-RES-12-004`, complete-considered-set normalization | `EFFECTIVE_WITH_NORMALIZATION` | any interpretation that equates the tied ambiguity subset with complete considered facts | An ambiguous Call retains the two tied candidates separately from the complete three-candidate considered set, performs all three probes, performs no selected replay, and publishes one retained argument fact. | `arcweft-lang-sema::final_analysis::tests::ambiguous_call_retains_complete_considered_set_beyond_the_tied_subset` | `PASS` | The exact owner passed within the same 62/62 final-analysis run on 2026-08-08 at Git HEAD `52b8c917632358d2360e0bb2ea5c32ecc7ca562b`; no nearby resolver test is substituted. |
| `CALL_ADJ_4_1_1_1_1_2_1` | Proof-witness portion of `T-RB-12-007` and repository-normalized conflict precedence | `EFFECTIVE_WITH_NORMALIZATION` | any retry ordering drift or considered-order selection of a rejected candidate before an ambiguity conflict | Repeating projection over unchanged complete facts returns the identical ordered witness payload. After the primary witness, the first distinct ambiguity conflict precedes an earlier rejected-but-considered candidate; three considered candidates retain two witnesses and one omission. This PASS is limited to verifier projection and does not promote the broader rollback transaction row. | verifier `proof_call_witness_order_is_deterministic_across_retry`; `ambiguity_conflict_precedes_an_earlier_rejected_candidate` | `PASS` | Both owners passed within `cargo test -p arcweft-verify --lib call_witness::tests:: -- --nocapture` (4/4) on 2026-08-08 at Git HEAD `52b8c917632358d2360e0bb2ea5c32ecc7ca562b` with the public-switch working copy dirty. |
| `FLOW_ADJ_2_1_1_1` | all 306 returned tests under adjudicated meaning | `EFFECTIVE_WITH_NORMALIZATION` | rejected ordinary-Flow return and conflicting rows in the redelivery | Shared callable/signature/scope/local owners; one no-tail `HirThreadBody`; all sixteen item variants in Flow and Thread contexts; missing Flow body publishes typed poison but no executable candidate. | syntax Flow attachment; HIR item Flow lowering; Thread statement/choice lowering; source-index Flow/Thread projection; sema/compiler/LSP/runtime-plan consumers. | `PASS` | Final syntax/HIR/sema suites, LSP typed navigation 10/10, runtime identity owners, and the real 65,536/65,537 Flow transaction passed on the coherent copy. |
| `FLOW_ADJ_2_1_1_1` | repository-adjudicated `E36 Choice` | `EFFECTIVE_WITH_NORMALIZATION` | leaf/source closed 35-expression-family count | Direct Choice and LetChoice share one source-backed `ChoiceExpression -> ExprId -> HirExprKind::Choice`; wrappers do not own a second payload. | syntax Flow/Choice attachment; HIR `expression_lowering/tests/choice.rs`; Flow/Thread item projection tests. | `PASS` | Direct and LetChoice owners passed within the final HIR suite; this remains repository adjudication, not an invented 83rd ZIP row. |
| `FLOW_ADJ_2_1_1_1` | `F06`, `T-ITEM-F-06`, `T-ITEM-T-06`, `T-ITEM-R-06`, and F06 portion of `T-BODY-09` | `EFFECTIVE_WITH_NORMALIZATION` | returned phrase “scope child of match statement scope” | Match has no container-wide Block scope. Scrutinee name lookup uses the inherited outer lexical scope while its once-evaluation extent remains Match-owned through the join. Each ordinary arm has a distinct outer-parented `MatchArm` typed-owned by the Match ID; only an authored BlockExpr value nests a Block. A braced Thread arm has one Block as both arm scope and nested Thread-body owner, with no parallel MatchArm. Match ownership is intentionally one semantic owner to multiple lexical arm scopes. | HIR Match expression/statement lowering, scope-graph freeze, Thread nested-body projection, and runtime once-evaluation/binding-cleanup tests. | `PASS` | Syntax Match passed 9/9 and the shared Flow/Thread sixteen-family projection passed 2/2. HIR Flow ordinal/source and malformed recovery passed 1/1 each; Thread scope/source/recovery passed 3/3; the wider HIR Match set passed 25/25. Core once-evaluation/cleanup and AWBC product parity each passed 1/1 on 2026-08-08. |
| `FLOW_ADJ_2_1_1_1` | `T-LIMIT-26` | `EFFECTIVE_WITH_NORMALIZATION` | returned claim that direct body items have no parallel limit | Shared `HirLimit::ThreadFlowItems` remains 65,536 inclusive; 65,537 rolls back the entire lowering transaction. | HIR Thread-body limit/transaction owner. | `PASS` | Exact production-boundary owner passed 1/1 in 85.23 seconds on 2026-08-09. |
| `FLOW_ADJ_2_1_1_1` | `T-CTR-07C` and source row `C07/Q10` | `EFFECTIVE_WITH_NORMALIZATION` | overloaded Keyword/Mode wording | `ensures no_effect` owns distinct `ClauseKeyword` and `NoEffectKeyword` source components plus its operand. | syntax Flow contract attachment; HIR Flow role/source tests. | `PASS` | Syntax Flow attachment passed 54/54 and directly distinguished the `ensures`, `no_effect`, and operand ranges. HIR Flow lowering passed 17/17 and proved the operand role resolves to the same typed Expr source while remaining distinct from both keyword sites on 2026-08-08. |
| `FLOW_ADJ_2_1_1_1` | `T-BODY-03` | `EFFECTIVE_WITH_NORMALIZATION` | contradictory final sentence of returned source row D08 | A recognized Flow missing its required body commits its item, exact scopes, stable empty body, insertion requirement, and `MissingBody` poison; it publishes no candidate. | syntax Flow attachment and HIR item Flow lowering. | `PASS` | The same syntax 54/54 and HIR 17/17 runs proved the typed zero-width insertion, committed item/source order, exact four-scope graph, stable empty body, recovered/non-executable/non-cacheable state, and `MissingBody` poison. Existing project/compiler executable-view gates reject that module before candidate publication. |

## Design-blocked role-specific rows

These are not ordinary missing tests. Current production has no legitimate
producer/payload authority, and fixture-only reservations receive no credit.
The remainder of Proof may continue without them.

| `authority_key` | `local_row_id` | `effective_contract_state` | `supersedes` | `normalized_expectation` | `test_owner_candidate` | `execution_state` | `evidence` |
|---|---|---|---|---|---|---|---|
| `TAIL_4_1_1_1_1` | `T-ROLE-07`, `T-GEN-TEMP-01`, `T-GEN-TEMP-02`, `T-GEN-TEMP-03`, `T-GEN-TEMP-STMT-01` | `DESIGN_BLOCKED` | fixture-only Pipe/source-scan reservation | The correction must select one complete production recipe/payload/consumer authority or delete `DesugaredTemporary` and all associated claims directly. | No legitimate current production test owner. | `BLOCKED` | [DesugaredTemporary design gap](2026-08-06-proof-desugared-temporary-production-recipe-design-gap.md) and its linked request. |
| `TAIL_4_1_1_1_1` | `T-ROLE-10` and the environment portion of `T-GEN-CAPTURE-03` | `DESIGN_BLOCKED` | unreferenced environment child assumption | The correction must define a real closure-environment payload/reference/consumer or delete the role and tag; ordered captures remain independently testable. | Capture lowering can test captures, but no legitimate environment-child owner exists. | `BLOCKED` | [ClosureEnvironment design gap](2026-08-06-proof-closure-environment-payload-consumer-design-gap.md) and its linked request. |
| `TAIL_4_1_1_1_1` | role-tag-dependent portions of `T-FP-02`, `T-FP-04`, and any fixed-vector assertion changed by retain/delete/renumbering | `DESIGN_BLOCKED` | current 21-role contiguous-tag claim | Rebaseline only after both role corrections select retention/deletion, tag continuity, and transcript domain/version. Independent owner-tag and pre-gap vector properties stay separate. | HIR identity/fingerprint tests after the design return. | `BLOCKED` | A current enum/tag gap or static vector is not compatibility or completion evidence. |

Match is not blocked by either role request. The normalized F06 row above owns
its complete scope graph. In tail row `AR-E32`, each missing arm value is owned
by that arm's already reserved ScopeId.

## Retained archive identities

Hashes below were recomputed from the retained repository bytes during this
audit.

| `authority_key` | Repository path | Bytes | SHA-256 | Intake state |
|---|---|---:|---|---|
| `BASE_6_1_1` | [`docs/reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip) | 104,901 | `1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF` | accepted |
| `LEAF_4_1` | [`docs/reviews/designs/proof-concurrency-v6.1.1.4.1/...zip`](../reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip) | 64,523 | `61E2EE166BFF158FE83DCF1484B7B9380A81F60D865377503400D27D238CC708` | retained where uncontradicted |
| `SOURCE_4_1_1` | [`docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1/...zip`](../reviews/designs/proof-concurrency-v6.1.1.4.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip) | 91,023 | `2BCD3F78EFB76442C2698A24251C4D874F7A941C5A8985649EA157100908A72E` | accepted correction |
| `SYNTHETIC_REJECTED_4_1_1_1` | [`docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1/...zip`](../reviews/designs/proof-concurrency-v6.1.1.4.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1-synthetic-role-owner-admission-correction-final-contract.zip) | 33,968 | `A9603B3CC758D95DADA69310F87A2DC26B7A2CE0EA8B6E0DE39DE4AA51E75024` | historical rejected return |
| `TAIL_4_1_1_1_1` | [`docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/...zip`](../reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip) | 50,036 | `69DC42FC7C985FED638D08D694ED301291A50AF3CEFA7117321D4219BE7E6471` | accepted correction |
| `SELECT_CENTRAL_4_1_1_1_1_1_1` | [`docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1.1.1.1/...zip`](../reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1.1-select-central-projection-and-accounting-correction-final-contract.zip) | 61,791 | `E20B646F8914E39E50456C164F2F6BF967376620571335297D3EFF42824213F4` | accepted with evidence normalization |
| `CALL_ADJ_4_1_1_1_1_2_1` | [`docs/reviews/packages/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction-final-contract.zip) | 38,626 | `41BB47824B91072B17EF79B8C50249863977AF5D5854EBCF9E515849C9F24480` | returned ZIP rejected; repository decisions release implementation |
| `FLOW_ADJ_2_1_1_1` | [`docs/reviews/packages/arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip) | 87,004 | `BDC55671E7D4F8CDB3D07D8EC004672C90E14DEA88A47E63D8189E585BB3E4DF` | standalone READY rejected; repository-adjudicated implementation released |

The earlier 1,305-byte final-leaf placeholder and the two rejected Select
returns have no normative matrix authority. They remain historical intake
evidence only.

## Evidence update procedure

For each effective local row or closed row group:

1. expand grouped IDs to exact `(authority_key, local_row_id)` rows;
2. compare the required behavior with the current typed owner;
3. run the smallest exact or explicitly equivalent focused test;
4. record the command, exact selected/pass/fail count, final Git SHA, and date;
5. use `PASS` only for that executed row, never for a nearby test or static
   symbol; and
6. after all focused rows are closed, record changed-crate checks/Clippy,
   workspace check/strict Clippy/tests, applicable Tier 2, and structure audit
   as separate cut-level evidence.

Superseded and non-applicable rows are not resurrected as compatibility tests.
Design-blocked rows remain blocked until their named correction returns one
complete retained or deleted authority.

## Validation performed for this note

Performed:

- opened the base, leaf, source-owner, rejected synthetic-role,
  tail/generator, Select, Call, and ordinary-Flow ZIP members directly;
- parsed every named TSV matrix and recomputed row/unique-ID counts;
- recomputed every retained ZIP SHA-256 shown above;
- compared package self-status with repository intake adjudication; and
- inspected current typed test-owner candidates; and
- on 2026-08-08, executed the syntax Flow attachment suite (54 passed) and HIR
  Flow item-lowering suite (17 passed) to close `T-CTR-07C`/`C07-Q10` and
  `T-BODY-03`;
- `cargo test -p arcweft-lang-sema --lib final_analysis::tests:: --
  --nocapture` passed 62/62, including the normalized Call two/three-candidate
  accounting, complete ambiguity set, and candidate-257 rollback owners; and
- `cargo check -p arcweft-verify --all-features` passed with 71 existing sema
  `dead_code` warnings, then `cargo test -p arcweft-verify --lib
  call_witness::tests:: -- --nocapture` passed 4/4 for exact two/three witness
  retention, deterministic retry projection, and conflict precedence.

Final coherent-copy validation added on 2026-08-09:

- syntax library 690/690 and syntax public-API trybuild passed;
- HIR library 841/841 executed with eight ignored Tier-2 owners; HIR
  public-API trybuild passed all 37 fixtures;
- all eight applicable real production-boundary Tier-2 owners passed
  individually; the Flow 65,536/65,537 owner completed in 85.23 seconds;
- sema library 163/163 passed;
- AW-AH-009.3 current owners passed: generated metadata 6/6, profile cache
  7/7, profile state 12/12, signature cache 42/42, signature/request 50/50,
  and positions 8/8;
- LSP typed entry-role navigation passed 10/10;
- runtime assertion, compile-fail, tooling projection, compiler reload, and
  AWFB save/import identity owners all passed;
- `cargo check --workspace --all-targets --all-features` passed;
- workspace Clippy passed for all targets and features with `-D warnings`; and
- `just structure-audit` plus `just structure-audit-gate` passed with zero
  blocking violations.

The complete LSP suite still has eight Character-definition failures because
the final compiler correctly rejects `show(...)` until the typed Presentation
command ABI is supplied by the unreturned
[`AW-AH-011/013` request](../reviews/requests/2026-07-14-aw-ah-011-and-013-typed-presentation-command-abi.md).
That dependency is outside this Proof rollup and is not a CharacterDialogue
contract gap; no fixture weakening, source fallback, compatibility alias, dual
reader, string runtime path, or Presentation shim is admitted.

The Proof rollup's role exclusions are the `DesugaredTemporary` and
`ClosureEnvironment` correction rows named above. The unrelated LSP suite
blocker is the unreturned typed Presentation ABI. The implementation infers
none of their role admission, payload, consumer, tags, fingerprint vectors, or
runtime command schema.
