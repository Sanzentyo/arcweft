# Exhaustive implementation test matrix

The status column distinguishes tests performed on this design archive from
production tests required of the implementation. `PACKAGE_PASS` means the
included validator/self-tests were actually run. `IMPLEMENTATION_REQUIRED`
means this design closes the expected behavior but no production patch exists
in this archive.

## 1. Included-role mutation and domain tests

| ID | Mutation/assertion | Expected | Status |
|---|---|---|---|
| INC-01 | mutate `RuntimeExecutableSemanticDigest` only | plan digest changes | IMPLEMENTATION_REQUIRED |
| INC-02 | mutate producer-function semantic input only | producer-function and plan digest change | IMPLEMENTATION_REQUIRED |
| INC-03 | change allowed producer family while holding other child fixtures | plan digest changes | IMPLEMENTATION_REQUIRED |
| INC-04 | change `TaskClass` | plan digest changes | IMPLEMENTATION_REQUIRED |
| INC-05 | mutate endpoint/static request child-role/type | request-template and plan digest change | IMPLEMENTATION_REQUIRED |
| INC-06 | mutate checked control/effect row | control/effect and plan digest change | IMPLEMENTATION_REQUIRED |
| INC-07 | mutate semantic binding tag/payload | plan digest changes | IMPLEMENTATION_REQUIRED |
| INC-08 | same seven fields under owner tag 0 vs accepted owner tag 1/2 fixture | domain/owner separation changes digest | IMPLEMENTATION_REQUIRED |
| INC-09 | task class and family tag methods cover every current enum variant exhaustively | compile + exact golden tags | IMPLEMENTATION_REQUIRED |

The request's “seven included roles” are executable, producer function, family,
class, request template, control/effect, and binding. Plan owner is fixed to
structured tag zero for this child and has the separate domain-separation test.

## 2. Explicit exclusion and legitimate-owner tests

| ID | Mutation | Plan digest | Other required consequence | Status |
|---|---|---:|---|---|
| EXC-01 | producer contract | unchanged | producer contract/instance key changes | IMPLEMENTATION_REQUIRED |
| EXC-02 | producer site | unchanged | producer instance key changes | IMPLEMENTATION_REQUIRED |
| EXC-03 | payload type | unchanged | runtime type/producer instance changes | IMPLEMENTATION_REQUIRED |
| EXC-04 | actual argument value | unchanged | canonical RuntimeValue/instance digest changes | IMPLEMENTATION_REQUIRED |
| EXC-05 | generation | unchanged | TaskKey/TaskId correlation changes | IMPLEMENTATION_REQUIRED |
| EXC-06 | policy | unchanged | Need/task correlation changes | IMPLEMENTATION_REQUIRED |
| EXC-07 | launch ordinal | unchanged | NeedId/TaskId changes under accepted rules | IMPLEMENTATION_REQUIRED |
| EXC-08 | priority | unchanged | TaskSpec scheduling comparison changes | IMPLEMENTATION_REQUIRED |
| EXC-09 | cancellation scope | unchanged | TaskSpec scheduling comparison changes | IMPLEMENTATION_REQUIRED |
| EXC-10 | debug label/origin | unchanged | diagnostic owner changes only | IMPLEMENTATION_REQUIRED |
| EXC-11 | decoded expected key | recomputed digest unchanged | decode returns expected-key mismatch | IMPLEMENTATION_REQUIRED |
| EXC-12 | accepted View revision only | unchanged | validated resource/replacement stamp changes | IMPLEMENTATION_REQUIRED |
| EXC-13 | source span/HIR arena allocation/source spelling only | all semantic child/plan digests unchanged | source map/debug owner changes | IMPLEMENTATION_REQUIRED |
| EXC-14 | arbitrary map insertion order with same canonical owner rows | unchanged | deterministic artifact equal | IMPLEMENTATION_REQUIRED |

## 3. Child encoder tests

| ID | Scenario | Expected | Status |
|---|---|---|---|
| CHD-01 | exact producer-function byte golden with params/captures/endpoints | exact bytes/hash | IMPLEMENTATION_REQUIRED |
| CHD-02 | body task reference uses build coordinate, not completed digest | exact byte golden/structural absence | IMPLEMENTATION_REQUIRED |
| CHD-03 | request positional/named/spread/capture/await/timeout/line roles | exhaustive exact tags/order | IMPLEMENTATION_REQUIRED |
| CHD-04 | request literal value bytes change but static role/type unchanged | request-template digest unchanged | IMPLEMENTATION_REQUIRED |
| CHD-05 | control modes 0..5 and effect kinds 0..6 | exhaustive exact tags/order | IMPLEMENTATION_REQUIRED |
| CHD-06 | scheduling metadata mutation | control/effect digest unchanged | IMPLEMENTATION_REQUIRED |
| CHD-07 | unknown reference/tag | typed error before digest | IMPLEMENTATION_REQUIRED |
| CHD-08 | direct canonical bytes vs direct hash visitor | equal grammar/hash | IMPLEMENTATION_REQUIRED |

## 4. Executable transcript and cycle tests

| ID | Scenario | Expected | Status |
|---|---|---|---|
| EXE-01 | one fixture row for every fixed table tag 0..14 | exact table/order golden | IMPLEMENTATION_REQUIRED |
| EXE-02 | mutate each table's first/last source-order role | executable digest changes | IMPLEMENTATION_REQUIRED |
| EXE-03 | mutate shadow task-plan map key | executable digest unchanged | IMPLEMENTATION_REQUIRED |
| EXE-04 | mutate shadow self digest | executable digest unchanged | IMPLEMENTATION_REQUIRED |
| EXE-05 | mutate shadow expected key | executable digest unchanged | IMPLEMENTATION_REQUIRED |
| EXE-06 | View program/site/admission/revision absent from core executable base | structural and byte golden | IMPLEMENTATION_REQUIRED |
| EXE-07 | task body reference coordinate changes | executable digest changes | IMPLEMENTATION_REQUIRED |
| EXE-08 | finite accepted nominal recursion | terminates through semantic leaf | IMPLEMENTATION_REQUIRED |
| EXE-09 | forbidden structural visiting cycle | deterministic cycle error | IMPLEMENTATION_REQUIRED |
| EXE-10 | repeated encode same candidate | byte-identical child/executable outputs | IMPLEMENTATION_REQUIRED |

## 5. Binding and View authority tests

| ID | Scenario | Expected | Status |
|---|---|---|---|
| BND-01 | Ordinary + StructuredTaskPlan | accepted tag 0 | IMPLEMENTATION_REQUIRED |
| BND-02 | View + ViewMatchSubscription with exact actual binding | accepted tag/payload 1 | IMPLEMENTATION_REQUIRED |
| BND-03 | AwaitManyBase | accepted tag 2 | IMPLEMENTATION_REQUIRED |
| BND-04 | AwaitManyChild | accepted tag 3 | IMPLEMENTATION_REQUIRED |
| BND-05 | Timeout exact contract | accepted tag/payload 4 | IMPLEMENTATION_REQUIRED |
| BND-06 | Line exact plan | accepted tag/payload 5 | IMPLEMENTATION_REQUIRED |
| BND-07 | HostAdapterTask/MakeNeedHandle Ordinary | accepted | IMPLEMENTATION_REQUIRED |
| BND-08 | AwbcTaskPlan inserted in structured owner | typed rejection | IMPLEMENTATION_REQUIRED |
| BND-09 | every mismatched family/binding pair | exhaustive typed rejection | IMPLEMENTATION_REQUIRED |
| VIEW-01 | ordinary-only finish with no authority | succeeds; no authority call | IMPLEMENTATION_REQUIRED |
| VIEW-02 | first View row with no authority | MissingViewTaskPlanAuthority, no publication | IMPLEMENTATION_REQUIRED |
| VIEW-03 | current authority missing coordinate | MissingBinding | IMPLEMENTATION_REQUIRED |
| VIEW-04 | foreign coordinate owner | CoordinateOwnerMismatch | IMPLEMENTATION_REQUIRED |
| VIEW-05 | binding program mismatch | ProgramMismatch | IMPLEMENTATION_REQUIRED |
| VIEW-06 | site mismatch | SiteMismatch | IMPLEMENTATION_REQUIRED |
| VIEW-07 | admission mismatch | AdmissionMismatch | IMPLEMENTATION_REQUIRED |
| VIEW-08 | accepted revision/source-set stale plus missing row | StaleAuthority wins | IMPLEMENTATION_REQUIRED |
| VIEW-09 | revision-only change with same actual semantic binding | plan digest equal | IMPLEMENTATION_REQUIRED |
| VIEW-10 | extra binding for non-View coordinate | complete product rejects before seal | IMPLEMENTATION_REQUIRED |
| VIEW-11 | duplicate binding coordinate | complete product rejects deterministically | IMPLEMENTATION_REQUIRED |
| VIEW-12 | authority finalizer cannot be called twice/without request | compile failure/move error | IMPLEMENTATION_REQUIRED |

## 6. Duplicate and expected-key tests

| ID | Scenario | Expected | Status |
|---|---|---|---|
| DUP-01 | identical Ordinary rows | second coordinate duplicate error | IMPLEMENTATION_REQUIRED |
| DUP-02 | private test fixture injects same typed bytes for Ordinary and View | global cross-family duplicate error | IMPLEMENTATION_REQUIRED |
| DUP-03 | same typed bytes for Timeout and Line test fixtures | global cross-family duplicate error | IMPLEMENTATION_REQUIRED |
| DUP-04 | one row referenced by multiple producer sites | accepted; one table row | IMPLEMENTATION_REQUIRED |
| KEY-01 | exact stored expected keys | decode succeeds | IMPLEMENTATION_REQUIRED |
| KEY-02 | first expected key tampered | first coordinate mismatch | IMPLEMENTATION_REQUIRED |
| KEY-03 | expected-key count shorter/longer | count mismatch before semantic hashing | IMPLEMENTATION_REQUIRED |
| KEY-04 | tamper and duplicate coexist | expected-key mismatch wins | IMPLEMENTATION_REQUIRED |
| KEY-05 | all-zero expected bytes equal genuine recomputation fixture | accepted (no zero sentinel) | IMPLEMENTATION_REQUIRED |

## 7. Exact-limit / one-over tests

For every row below, construct an otherwise valid fixture at the exact limit and
one over. Exact passes; one over returns the named limit before publication.

| ID | Limit |
|---|---|
| LIM-01 | task plan rows 65,536 / 65,537 |
| LIM-02 | executable rows 1,048,576 / 1,048,577 |
| LIM-03 | children per row 65,536 / 65,537 |
| LIM-04 | function roles 65,536 / 65,537 |
| LIM-05 | request roles 65,536 / 65,537 |
| LIM-06 | control/effect rows 65,536 / 65,537 |
| LIM-07 | View bindings 65,536 / 65,537 |
| LIM-08 | transcript bytes 67,108,864 / 67,108,865 |
| LIM-09 | semantic work 4,194,304 / 4,194,305 |
| LIM-10 | checked count addition overflow fixture | ArithmeticOverflow before limit |
| LIM-11 | string/list `usize -> u32` overflow fixture | ArithmeticOverflow before write |

All are IMPLEMENTATION_REQUIRED.

## 8. Atomicity and publication tests

| ID | Failure injection point | Required observation | Status |
|---|---|---|---|
| ATM-01 | structural verification | no RuntimePlan/table/iterator observed | IMPLEMENTATION_REQUIRED |
| ATM-02 | producer-function child | no publication | IMPLEMENTATION_REQUIRED |
| ATM-03 | request child | no publication | IMPLEMENTATION_REQUIRED |
| ATM-04 | control/effect child | no publication | IMPLEMENTATION_REQUIRED |
| ATM-05 | executable row | no publication | IMPLEMENTATION_REQUIRED |
| ATM-06 | middle View authority row | earlier sealed rows remain private/dropped | IMPLEMENTATION_REQUIRED |
| ATM-07 | expected-key mismatch | no RuntimePlan or View resource publication | IMPLEMENTATION_REQUIRED |
| ATM-08 | duplicate | no publication | IMPLEMENTATION_REQUIRED |
| ATM-09 | final cross-reference | no publication | IMPLEMENTATION_REQUIRED |
| ATM-10 | complete valid bundle | RuntimePlan + View resource become visible together | IMPLEMENTATION_REQUIRED |

## 9. Compile-fail and structural tests

| ID | Forbidden code | Expected | Status |
|---|---|---|---|
| NEG-01 | struct literal for `RuntimeTaskPlanDigestBase` | private-field compile failure | IMPLEMENTATION_REQUIRED |
| NEG-02 | `clone()` on base/request | trait-bound compile failure | IMPLEMENTATION_REQUIRED |
| NEG-03 | serde serialize/deserialize base/request | trait-bound compile failure | IMPLEMENTATION_REQUIRED |
| NEG-04 | `TaskPlanSemanticDigest::from_bytes`/`From<[u8;32]>` | no API compile failure | IMPLEMENTATION_REQUIRED |
| NEG-05 | raw core View projection type | unresolved type/structural gate | IMPLEMENTATION_REQUIRED |
| NEG-06 | `RuntimeTaskPlan { semantic_digest: ... }` | unknown field compile failure | IMPLEMENTATION_REQUIRED |
| NEG-07 | caller digest argument to builder/lowering | signature compile failure | IMPLEMENTATION_REQUIRED |
| NEG-08 | generic sink/callback API | rustdoc/API structural gate | IMPLEMENTATION_REQUIRED |
| NEG-09 | extension trait/free family tag helper | structural absence gate | IMPLEMENTATION_REQUIRED |
| NEG-10 | core Cargo dependency on View/bundle | Cargo metadata gate failure | IMPLEMENTATION_REQUIRED |
| NEG-11 | generic Serde RuntimeTaskPlan decode | trait/API failure | IMPLEMENTATION_REQUIRED |
| NEG-12 | second task-plan table/catalog | structural owner gate failure | IMPLEMENTATION_REQUIRED |

## 10. Focused implementation commands

```text
cargo fmt --all -- --check
cargo test -p arcweft-core task_plan_semantic
cargo test -p arcweft-runtime-plan task_plan_semantic
cargo test -p arcweft-compiler view_match_admission
cargo test -p arcweft-bundle validated_view_task_plan
cargo test -p arcweft-bundle runtime_plan_expected_key
cargo test -p arcweft-core --test task_plan_compile_fail
cargo metadata --format-version 1 --no-deps
cargo clippy --all-targets --all-features -- -D warnings
```

The repository's current AGENT-selected broader suites and deterministic
artifact generation must also run. Generate the same plan/bundle artifacts at
least twice from clean target/output directories and compare bytes and hashes.
Record exact commands, SHA, platform/toolchain, and outcomes.

## 11. Archive validator tests actually performed

| ID | Scenario | Status |
|---|---|---|
| PKG-01 | manifest/hash/member validation on extracted package | PACKAGE_PASS |
| PKG-02 | same validation on returned ZIP | PACKAGE_PASS |
| PKG-03 | wrong final status mutation rejected | PACKAGE_PASS |
| PKG-04 | non-none open question mutation rejected | PACKAGE_PASS |
| PKG-05 | manifest payload tamper rejected | PACKAGE_PASS |
| PKG-06 | missing authoritative transcript line rejected | PACKAGE_PASS |
| PKG-07 | self-digest field mutation rejected | PACKAGE_PASS |
| PKG-08 | public raw digest constructor mutation rejected | PACKAGE_PASS |
| PKG-09 | raw core View projection mutation rejected | PACKAGE_PASS |
| PKG-10 | caller sink/callback mutation rejected | PACKAGE_PASS |
| PKG-11 | expected digest made public field rejected | PACKAGE_PASS |
| PKG-12 | machine dependency changed to core->View rejected | PACKAGE_PASS |
| PKG-13 | version marker changed from one rejected | PACKAGE_PASS |
| PKG-14 | archive path traversal/case-fold/duplicate checks | PACKAGE_PASS |
