# Test plan

| Test ID | Decisions | Layer | Fixture | Exact oracle |
|---|---|---|---|---|
| GM-T001 | D03,D04 | unit | three source arms including an or-pattern | exactly three arm rows; alternatives 1/2/1; stable IDs and source paths |
| GM-T002 | D02,D18 | negative unit | builder missing middle arm | `MissingArmRow { ordinal: 1 }`; no complete carrier |
| GM-T003 | D05 | negative compile/check | generic substitution retains inference variable | `UnresolvedGenericUniverse`; no HIR match |
| GM-T004 | D06 | negative compile/check | finite enum leaves one constructor uncovered | structured `CoverageGapWitness`; `Open` cannot convert to complete |
| GM-T005 | D06,D19 | golden | finite enum fully covered without wildcard | closed bitset, witnesses and transcript digest match golden bytes |
| GM-T006 | D07 | unit | only guarded arm for a constructor, guard unknown | constructor remains uncovered |
| GM-T007 | D07 | unit | guard proven true | constructor closes coverage |
| GM-T008 | D07,D13 | negative | guard proven false | alternative retained as unreachable/redundant with stable diagnostic code |
| GM-T009 | D09,D10 | property | same semantic arms inserted through permuted temporary map order | identical canonical bytes and digest |
| GM-T010 | D10 | negative decode | unknown schema version | decode fails before catalog insertion |
| GM-T011 | D10,D12 | negative restore | single bit flipped in body | digest mismatch; no runtime handle published |
| GM-T012 | D08,D14 | unit | runtime Need atom and view identity round trip | original owner impl ordering/encoding is used; identity is exact |
| GM-T013 | D08,D12 | negative restore | Need identity absent from restored catalog | dangling identity error; no partial task |
| GM-T014 | D11 | compile-fail/API | call lowering with builder/open coverage | type mismatch; only `&CompleteMatchTranscript` accepted |
| GM-T015 | D13 | golden diagnostics | two missing constructors and one redundant alternative | stable codes, anchors, witness order |
| GM-T016 | D15 | benchmark | 10k alternatives over finite canonical universe | sealing linear in rows/atoms; runtime dispatch unchanged |
| GM-T017 | D16,D17 | cache | same owner with two generic substitutions | distinct keys/transcripts; no cross-instantiation reuse |
| GM-T018 | D16,D17 | cache | source span changes only | semantic digest unchanged when existing stable owner policy allows |
| GM-T019 | D20 | migration | legacy snapshot has no transcript schema tag | fail-closed rejection and regeneration path |
| GM-T020 | D03,D04,D19 | property | generated well-typed match AST | every source arm/normalized alternative has one unique transcript row |
| GM-T021 | D06,D19 | property | generated finite constructor universes | closed iff covered set equals canonical universe |
| GM-T022 | D09,D10 | cross-platform golden | encode on all supported targets | byte-for-byte identical transcript and closure digests |
| GM-T023 | D12 | integration | decode succeeds but closure universe digest differs from checked catalog | candidate rejected before task visibility |
| GM-T024 | D14 | source structure lint/review | search for extension traits/ad-hoc Need encoders introduced by change | zero; behavior lives on original enum impl |
| GM-T025 | D01,D11 | integration | attempt runtime reconstruction without checker transcript | no public API exists; admission error at internal boundary |
| GM-T026 | D03,D13 | negative internal invariant | duplicate arm ID or duplicate normalized ordinal | distinct invariant error; decode/seal rejection |
| GM-T027 | D04 | unit | nested or-pattern with bindings | reversible canonical pattern paths and binding slots preserved |
| GM-T028 | D06,D13 | unit | wildcard after complete constructor coverage | closed coverage plus redundancy witness for wildcard |
| GM-T029 | D18 | negative | parser recovery arm reaches checker | poisoned transcript; no digest, HIR, or runtime plan |
| GM-T030 | D11,D12,D20 | end-to-end | compile, persist, restore, dispatch generic match | same transcript ID/coverage digest before and after restore |
| GM-T031 | D21 | unit/golden | alternative binds a typed Need producer | all six semantic identities and exact output type appear in row and golden bytes |
| GM-T032 | D21,D23 | negative verify | producer output type or runtime view differs from checked row | distinct producer/type/view mismatch; no HIR/runtime plan |
| GM-T033 | D22 | unit/golden | two AWBC lanes in one generic match | canonical allocation IDs, generation, lanes, and storage tags remain distinct and ordered |
| GM-T034 | D22,D26 | negative restore | restore substitutes a fresh AWBC allocation for persisted binding | verification rejects substitution before handle publication |
| GM-T035 | D23 | negative unit | same Need identity paired with wrong instance | instance mismatch diagnostic and fail-closed admission |
| GM-T036 | D24 | coverage | structural record shape equals fields of nominal constructor | does not close nominal universe without nominal carrier atom |
| GM-T037 | D25 | completeness audit | delete each transcript field in a mutation fixture | corresponding completeness/digest/reference test fails; no side-channel reconstruction |
| GM-T038 | D26 | end-to-end isomorphism | compile/persist/restore typed producer match | all transcript-bound runtime identities and digests are identical |
| GM-T039 | D27 | API/source audit | remove coverage certificate and attempt runtime fallback | no fallback API/path; explicit certificate-missing failure |
| GM-T040 | D28 | concurrency/generation | same generic source compiled in two generations | no mutable allocation alias; cache reuse only under complete key equality |

All rows are mandatory for implementation admission. A row is complete only when its exact oracle is asserted; pass/fail-only smoke tests do not close transcript or digest requirements.
