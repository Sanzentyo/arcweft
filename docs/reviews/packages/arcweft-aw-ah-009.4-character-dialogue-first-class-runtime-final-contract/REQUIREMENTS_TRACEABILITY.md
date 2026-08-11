# Requirements traceability

Every source-request obligation is mapped to one frozen decision and direct
implementation/test evidence. `CLOSED` means the contract contains no remaining
choice for the implementer; it does not claim production code is already
changed.

## 1. Direction and source surface

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| direct `Character[...]` and colon forms | D012, D026 | `FINAL_CONTRACT.md` §§1,10; `GRAMMAR_HIR_SEMA.md` §§1-3,15 | `SYN-001..004`, `TOL-001..006` | CLOSED |
| Character/CharacterDialogue `(...)` creates/reconfigures an immutable value | D001, D006, D012, D031 | `FINAL_CONTRACT.md` §§4,6,10; merge table | `SEM-*`, `CFG-*`, `RUN-*` | CLOSED |
| brackets/colon apply content; `with` attaches plan | D018 | `GRAMMAR_HIR_SEMA.md` §§1-4; `FINAL_CONTRACT.md` §9 | `SYN-*`, `HIR-*`, `RUN-*` | CLOSED |
| `.say` has no dialogue method meaning | D028, D030 | `DELETION_MATRIX.md`; `TOOLING_DIAGNOSTICS_LIMITS.md` §§1-2,10 | `SYN-016`, `SEM-034`, `DEL-004..012` | CLOSED |

## 2. Required decision 1 — exact owned types

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| exact `CharacterDialogue` shape and CharacterId ownership | D004, D005, D007 | `FINAL_CONTRACT.md` §§3-5 | `CFG-*`, `RUN-*`, `VAL-*` | CLOSED |
| exact effective config/patch shapes | D006-D010, D031 | `FINAL_CONTRACT.md` §4; `TYPE_AND_MERGE_TABLE.md` | `CFG-001..046` | CLOSED |
| exact runtime carrier | D001-D003 | `FINAL_CONTRACT.md` §§2,4-5; `RUNTIME_WIRE_PERSISTENCE.md` §§3-7 | `RUN-*`, `PER-*` | CLOSED |
| exact content-application carrier | D009, D018 | `FINAL_CONTRACT.md` §§4,7,9 | `HIR-*`, `RUN-*` | CLOSED |
| acyclic crate ownership | D004-D005 | `FINAL_CONTRACT.md` §3; `REPOSITORY_EVIDENCE.md` | `DEL-013`, structural audit | CLOSED |

## 3. Required decision 2 — configuration and merge

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| all standard fields covered | D007-D010 | `TYPE_AND_MERGE_TABLE.md` §3 | one field family per `CFG-*` row | CLOSED |
| reusable versus content-application placement | D009 | merge table §§1,3,9; final contract §7 | application-context tests | CLOSED |
| absent/default/clear states | D008 | merge table §§2-3,8; final contract §6 | exact/clear/preserve tests | CLOSED |
| scalar later-wins/preserve | D008, D031 | merge table §§2-3 | standard-field merge tests | CLOSED |
| structured leaf merge | D008, D031 | merge table §6 | structured merge/conflict tests | CLOSED |
| custom distinct/same-key rules | D010 | merge table §7 | custom-field tests | CLOSED |
| immutable Character identity | D005, D030 | merge table §10 | `CFG`/diagnostic/compile-fail rows | CLOSED |
| no canonical `BTreeMap<String,String>` | D010 | merge table §§3,7; runtime nominal layout | type/layout/codec tests | CLOSED |
| source provenance retained separately | D011 | merge table §11; grammar/HIR facts | hover/cascade/source-map tests | CLOSED |

## 4. Required decision 3 — runtime model

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| choose one model | D001 | `FINAL_CONTRACT.md` §1 | package status verification | CLOSED: RUNTIME_VALUE |
| reject static-elimination alternative with examples | D001-D002 | `FINAL_CONTRACT.md` §1 | branch/return/capture tests | CLOSED |
| ordinary aliases/branches/returns/closures/collections/indirect calls | D014 | final contract §§1,8,12; grammar/sema §10 | `SEM-*`, `RUN-*`, `PER-*` | CLOSED |
| preserve ordinary function/currying substrate | D002 | final contract §§2,10; repository evidence §5 | function regression suite | CLOSED |
| no hybrid string/static preset fallback | D001, D030 | runtime-plan verifier and deletion matrix | `RUN-*`, `DEL-010` | CLOSED |

## 5. Required decision 4 — grammar/CST/AST/HIR

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| Character and CharacterDialogue config calls | D012 | `GRAMMAR_HIR_SEMA.md` §§1,3 | syntax/call surface tests | CLOSED |
| bracket/colon/with nodes and ranges | D012, D018 | grammar/HIR §§2-4,7 | `SYN-*`, `HIR-*` | CLOSED |
| incomplete/malformed recovery | D012 | grammar §4 | malformed matrix | CLOSED |
| indexing/collection/record ambiguity | D012 | grammar §§5-6 | ambiguity tests | CLOSED |
| no `.say`/callee-string dependency | D028, D030 | grammar §§2,4,7,16 | HIR structural/deletion tests | CLOSED |

## 6. Required decision 5 — HIR/sema/shared resolver

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| new exact/any semantic type | D013 | final contract §8; grammar/sema §§8-10 | join/assignability tests | CLOSED |
| Character factory typing | D012-D013 | grammar/sema §9 | resolver/type tests | CLOSED |
| same-type reconfiguration | D012-D013 | grammar/sema §9 | immutable payload tests | CLOSED |
| content application/result typing | D018 | grammar/sema §§9,12; final contract §9 | plan result/effect tests | CLOSED |
| aliases/imports/branches/generics/closures/indirect calls | D014 | grammar/sema §§8-10 | semantic/runtime matrix | CLOSED |
| structured mismatch diagnostics | D013 | tooling/diagnostics §10 | diagnostic code/range tests | CLOSED |
| shared callable catalog publication | D012 | final contract §10; grammar/sema §§11-13 | resolver/signature-help tests | CLOSED |
| config versus content signature help | D012, D026 | grammar/sema §13; tooling §5 | LSP matrix | CLOSED |
| delete Speaker classifications | D030 | grammar/sema §16; deletion matrix | `DEL-*` | CLOSED |

## 7. Required decision 6 — line identity

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| choose line family | D015 | final contract §11 | ID family tests | CLOSED: retain `@say.*` |
| no relation to method spelling | D015-D016, D028 | final contract §11; deletion matrix | rename/generated ID tests | CLOSED |
| source-site generated IDs | D016 | grammar/sema §14 | exact owner/scope/ordinal tests | CLOSED |
| explicit/relative IDs and collision rules | D015-D017 | grammar/sema §14 | collision/relative/family tests | CLOSED |
| text-key derivation | D017 | final contract §11; grammar §14 | text-key tests | CLOSED |
| rename/save/replay identity | D015-D017 | tooling §7; runtime/persistence §§11-14 | rename/save/replay tests | CLOSED |

## 8. Required decision 7 — runtime-plan/verifier/execution

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| typed factory/patch/runtime expressions | D001, D006, D031 | runtime/wire §§1-2 | runtime-plan tests | CLOSED |
| typed content/line-plan lowering | D018, D021-D022 | runtime/wire §§1,6,8 | suspension/result tests | CLOSED |
| typed Character/View/voice/look/stage/style/Fx | D005, D007 | merge table; runtime frame/product validation | parity/effective-config tests | CLOSED |
| deterministic invocation/equality/hash/capture/effects/budgets | D014, D031-D032 | final contract §12; limits §§12-13 | VM/hash/limit tests | CLOSED |
| delete preset reconstruction | D030 | deletion matrix | `DEL-010`, runtime dynamic tests | CLOSED |

## 9. Required decision 8 — every real wire/persistence boundary

| Boundary | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| HIR/compiler query cache | D011 | runtime/wire §16 | accepted-generation/stale query tests | CLOSED: in-memory only |
| runtime plan | D001, D006 | runtime/wire §§1-2 | verifier/lowering tests | CLOSED: typed in-memory |
| AWBC constants/types/instructions | D003, D019, D021 | runtime/wire §§3-7 | deterministic codec/tamper tests | CLOSED: ABI2/codec8 |
| bundle display catalog | D022 | runtime/wire §§8-9 | schema2 codec/cross-validation tests | CLOSED |
| atomic patch/hot reload | D024 | runtime/wire §§10,14-15 | compatible/stale/reject-atomically tests | CLOSED |
| save snapshot | D020, D025 | runtime/wire §11 | schema2 round-trip/tamper/stale tests | CLOSED |
| root replay/debug trace | D025 | runtime/wire §12 | generic nominal payload and typed debug observation tests | CLOSED: root replay v1 retained |
| Agent observation | D023 | runtime/wire §13 | protocol/native/Web/Agent tests | CLOSED |
| source provenance | D011 | runtime/wire §§1-5,16 | source-map tests | CLOSED: sidecar only |
| old preset wire | D025, D030 | runtime/wire §16; deletion matrix | old bytes/payload rejection | CLOSED: no representation |

## 10. Required decision 9 — tooling

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| formatter | D027 | tooling §1; grammar §15 | formatter tests | CLOSED |
| colon canonicalization to brackets | D026 | tooling §2; grammar §15 | exact edit tests | CLOSED |
| completion/hover/signature help | D012, D026 | tooling §§3-5 | LSP matrix | CLOSED |
| definition/rename | D005, D015-D017 | tooling §§6-7 | multi-module/alias/reference tests | CLOSED |
| semantic tokens/code actions | D026-D028 | tooling §§8-9 | LSP negative/positive tests | CLOSED |
| accepted HIR/sema identity only | D011-D012 | tooling §§2-7 | stale/missing fact tests | CLOSED |
| never emit `.say` | D026-D028 | tooling §§1-2,5,9 | `TOL-*`, `DEL-011..012` | CLOSED |

## 11. Ownership, diagnostics, limits, order, and validation

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| lower-layer ownership/Sans I/O/no cycles | D004-D005 | final contract §3; repository evidence | Cargo metadata/structural audit | CLOSED |
| stable compiler/runtime diagnostics | D013, D021, D024 | tooling §§10-11 | exact code/field/range tests | CLOSED |
| all requested limits | D031-D032 | tooling §§12-13 | exact/one-over/encoded/nesting tests | CLOSED |
| no spelling-specific removed diagnostic | D028 | grammar §4; tooling §10 | ordinary method diagnostic test | CLOSED |
| implementation order | all | `IMPLEMENTATION_ORDER.md` | per-cut gates | CLOSED |
| required complete test matrix | all | `TEST_MATRIX.md` | 260 direct rows | CLOSED |
| structural audit | D004, D030 | implementation order Cuts 1-8 | `VAL-007` | CLOSED |

## 12. Required deletions and prohibitions

| Requirement | Frozen decision | Normative location | Test/gate | Status |
|---|---|---|---|---|
| every explicitly named old item deleted | D030 | `DELETION_MATRIX.md` §1 | `DEL-001..012` | CLOSED |
| additional string/suffix/tooling residue deleted | D030 | deletion matrix §§2-3 | dynamic/codec/tooling tests | CLOSED |
| no compatibility shim/dual reader | D025, D030 | deletion matrix §§3-4; final contract §15 | old API/byte tests | CLOSED |
| no source gate | D030 | deletion matrix §4; test matrix policy | architecture/API/codec tests | CLOSED |
| no CSS/Takumi | D033 | final contract §15; implementation global constraints | Cargo metadata audit | CLOSED |
| no redesign of verified substrate absent defect | D002, D005, D029 | final contract §2; repository evidence §§4-5 | regression suites | CLOSED |

## 13. Package acceptance

| Acceptance item | Evidence | Status |
|---|---|---|
| one nominal identity-bearing configured value | D001-D007; exact domain/runtime layout | CLOSED |
| CharacterId never recovered from spelling/line ID | D005, D011-D016; HIR/runtime shapes | CLOSED |
| complete merge table | `TYPE_AND_MERGE_TABLE.md` | CLOSED |
| runtime/static choice unambiguous | D001; rejected alternative in final contract | CLOSED |
| every real wire decided | runtime/wire §§3-17 and this traceability §9 | CLOSED |
| line family settled | D015-D017 | CLOSED |
| `.say`-free canonical tooling | D026-D028 | CLOSED |
| old path deleted without residue | D025, D030; deletion matrix | CLOSED |
| implementation order and tests complete | Cuts 1-8; 260 rows | CLOSED |
| no unresolved decisions | D034; `OPEN_QUESTIONS.md` | `OPEN_QUESTIONS=0` |
