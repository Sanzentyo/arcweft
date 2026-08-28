# Test matrix

The machine validator checks this table against
`machine/final_contract.json`. Implementation acceptance uses typed behavior,
compile checks, codecs/transcripts, and structured dependency graphs; source
searches are review aids only.

## Exhaustive 35-family producer matrix

| index | HIR tag | `HirStmtKind` | required final payload | detail |
| ---: | ---: | --- | --- | --- |
| 0 | `0x0700` | Assertion | `Assertion` | checked assertion disposition |
| 1 | `0x0701` | Let | `Structural` | child facts carry binding/initializer meaning |
| 2 | `0x0702` | Assign | `Assignment` | typed assignment |
| 3 | `0x0703` | LetElse | `Structural` | typed children/body carry meaning |
| 4 | `0x0704` | LetChoice | `Structural` | typed children carry meaning |
| 5 | `0x0705` | LetScope | `Structural` | typed children carry meaning |
| 6 | `0x0706` | LetActionReceive | `Structural` | typed children carry meaning |
| 7 | `0x0707` | Return | `Structural` | typed child/function contract carry meaning |
| 8 | `0x0708` | Out | `ControlTransfer` | `CheckedControlTransferTarget::Output` |
| 9 | `0x0709` | Goto | `Structural` | typed target child/callable fact carries meaning |
| 10 | `0x070A` | DeferBlock | `Defer` | checked defer outcome |
| 11 | `0x070B` | Defer | `Defer` | checked defer outcome |
| 12 | `0x070C` | Yield | `Yield` | expression type checked against StreamFactory item and proof consumed |
| 13 | `0x070D` | Signal | `Structural` | typed target/value children carry meaning |
| 14 | `0x070E` | LifetimeSet | `Structural` | typed children carry meaning |
| 15 | `0x070F` | Wait | `Suspension` | checked suspension; mark wait rejects in this cut |
| 16 | `0x0710` | On | `Trigger` | closed checked Trigger |
| 17 | `0x0711` | UnsafeLifetime | `UnsafeAudit` | typed unsafe ID + SAFETY bit |
| 18 | `0x0712` | Choice | `Structural` | typed children/body carry meaning |
| 19 | `0x0713` | If | `Structural` | typed condition/bodies carry meaning |
| 20 | `0x0714` | IfLet | `Structural` | typed scrutinee/pattern/bodies carry meaning |
| 21 | `0x0715` | Match | `Structural` | generic Match facts/transcript own non-child Match meaning |
| 22 | `0x0716` | While | `Structural` | typed condition/body carry meaning |
| 23 | `0x0717` | WhileLet | `Structural` | typed scrutinee/pattern/body carry meaning |
| 24 | `0x0718` | For | `Iteration` | checked iteration |
| 25 | `0x0719` | Close | `Structural` | typed child carries meaning |
| 26 | `0x071A` | Select | `Select` | Operand or source-ordered branch heads |
| 27 | `0x071B` | SourceLocale | `SourceLocale` | accepted `LocaleTag` |
| 28 | `0x071C` | Scope | `Scope` | Anonymous/Named presence; body coordinate is identity |
| 29 | `0x071D` | Include | `Include` | accepted Flow `CallableDeclarationDigest` |
| 30 | `0x071E` | Break | `ControlTransfer` | `CheckedControlTransferTarget::Loop` |
| 31 | `0x071F` | Continue | `ControlTransfer` | `CheckedControlTransferTarget::Loop` |
| 32 | `0x0720` | Expression | `EvaluatedEffect` or `Structural` | EvaluatedEffect only for exact sealed operation; otherwise Structural |
| 33 | `0x0721` | ProofCall | `Structural` | checked proof-call child/call facts carry meaning |
| 34 | `0x0722` | Error | rejection | sole rejection-only HIR family |

The generated matrix test constructs each enum variant through legitimate HIR
builders, analyzes it, and asserts exactly this row. It then mutates the
expected payload once per row. All 34 success mutations and an attempted Error
success must fail; using Structural for any non-whitelisted row must fail.

## Positive behavior

| ID | Fixture | Required assertions |
| --- | --- | --- |
| P01 | Trigger Input | pattern/local type is exact registered `Ref<Input>`; body publication succeeds |
| P02 | Trigger Event | one reachable stateful Entry event type is selected; completed pattern matches; Entry seal consumes proof |
| P03 | Trigger Signal without value | target is exact Signal-with-value; no fabricated pattern/local |
| P04 | Trigger Signal with value | optional pattern/local type equals exact target payload `T` |
| P05 | Trigger Timeout | checked expression is Duration; unit Trigger payload |
| P06 | Trigger Mark | no pattern/local child; stable coordinate equals catalog mark; runtime ID projection succeeds |
| P07 | Trigger Select | exact enclosing Choice lifecycle; pattern is `Ref<ChoiceOption>` |
| P08 | Trigger Task | pattern/local is exact registered TaskEvent ingress atom |
| P09 | Trigger Scope | pattern/local is exact registered ScopeExit ingress atom |
| P10 | Trigger Expression | checked expression is Bool; unit Trigger payload |
| P11 | nested/two marks | nested line-plan groups resolve two source-ordered marks without handler side table |
| P12 | same local mark name | equal local name in two applications yields different stable coordinates |
| P13 | mark runtime projection | content order issues expected contiguous `RuntimeDialogueMarkId`; temporary map is absent after publication |
| P14 | coordinated rename | marker and all uses renamed: coordinate and semantic digest equal; diagnostic name changes |
| P15 | reorder/reference swap | mark reorder or Trigger reference swap changes coordinate/digest as appropriate |
| P16 | Select Operand | payload tag Operand; typed operand child is present once |
| P17 | Select branches | Bind/Frame/Event retain source order; each binding/pattern/local/body and array index matches HIR roles; no ordinal field |
| P18 | prefix Try | `value = try source =>` uses checked Try child; removing Try changes source/statement digest; no Select bit |
| P19 | absolute unsafe | accepted `@unsafe.*`, String reason, exact semantic ID, SAFETY present/absent policy diagnostics from checked facts |
| P20 | unsafe repair | verifier/CLI action uses checked ID and typed child source site, never reparses rendered ID |
| P21 | Entry convergence | two Entries reaching one statement with equal event semantic type succeed independent of traversal order |
| P22 | recursive reachability | recursive/SCC call graph terminates and preserves complete Entry set; Include edge participates |
| P23 | perturb raw evidence | HIR arena allocation, raw IDs, spans, and harmless source formatting differ while mark/unsafe/Trigger/Select/statement/body digests remain equal |
| P24 | all-35 generator | one payload/rejection per table row; only Error rejects by family |
| P25 | consumer integration | compiler/runtime-plan/verifier use final checked rows without source spelling or public structural ExprId |
| P26 | environment digest | changing any exact ingress ID mapping changes registered environment digest; identical reordered input rows either canonicalize identically or duplicate-reject as specified |
| P27 | stale inventory correction | AST inventory proves five current expression `CheckedSelectResolution` variants and 26 style values; statement Select remains a distinct two-variant carrier |
| P28 | wait mark rejection | surface form cannot reach executable runtime plan and legacy String target is never constructed |

## Negative behavior

| ID | Rejected condition |
| --- | --- |
| N01 | mark selector malformed, missing dot, attributed, multiple, missing, duplicate, unknown, cross-application, recovered, or outside dialogue application |
| N02 | forged HIR mark has foreign content/tag, noncontiguous ordinal, duplicate tag/name, wrong application owner, stale module/generation, or recovery tag |
| N03 | Select trailing `source?`; parser neither strips it nor records hidden propagation state |
| N04 | recovered Select head or Trigger reaches checked construction |
| N05 | Select child/body branch index differs from source-order array index |
| N06 | Select Bind source/local type mismatch; Frame/Event roles swapped |
| N07 | missing, duplicate, mismapped, open, recovered, poison, Named, or conflicting standard ingress publication |
| N08 | zero reachable stateful Entry events, recovered Entry member/root, missing prepared type, or incompatible reachable Entry event schemas |
| N09 | wrong Input/Event/Select/Task/Scope/Frame pattern or local type |
| N10 | non-Signal target, Signal without value type, or wrong Signal payload pattern |
| N11 | non-Duration Timeout or non-Bool Expression trigger |
| N12 | zero/multiple/wrong/recovered enclosing Choice lifecycle for Trigger Select |
| N13 | unsafe ID relative, family-relative, wrong family, malformed, missing, recovered, or forged semantic-ID bytes |
| N14 | unsafe reason non-String or effectful; release policy still rejects missing reason/SAFETY as configured |
| N15 | missing, duplicate, foreign, stale, or unconsumed control-transfer/evaluated-effect application evidence |
| N16 | any per-row all-35 payload mutation, Structural on non-whitelist, wildcard success, or successful Error |
| N17 | stable mark coordinate collision, absent coordinate in owning compiler content map, duplicate runtime projection, or map retained after publication |
| N18 | runtime-plan receives a raw HIR mark/name, stable sema coordinate, HIR trigger reclassification, or legacy wait-mark String |
| N19 | statement/direct-child/body transcript has missing/extra/duplicate/stale role, role/ordinal swap, recovery row, wrong payload tag, or raw ID/spelling |
| N20 | transcript/HIR type contract attempts a version other than 1, serde fallback, `Other`, `UnsupportedIdentity` success, or whole-catalog digest |
| N21 | Entry reachability queue/counter/preallocation overflow or traversal N+1 beyond injected test limit; no partial proof/report |
| N22 | mark catalog, Select branch inventory, or transcript at N+1; exact N succeeds and no partial content/fact/digest publishes on failure |
| N23 | final Entry catalog event digest differs from consumed preparation proof |
| N24 | compiler/verifier/project index asks old `CheckedStatementRole` or re-reads HIR to select meaning |

## Compile/API unavailability gates

Trybuild/compile-fail fixtures prove that downstream code cannot:

- construct `StableCheckedDialogueMarkCoordinate`, `CheckedDialogueMark`,
  `CheckedTrigger`, `CheckedSelectStatement`, `CheckedUnsafeAudit`,
  `CheckedStatement`, `CheckedIncludeFlowTarget`, or runtime admission directly;
- name `HirTriggerPattern`, `CheckedStatementRole`,
  `CheckedDialogueMarkOrdinal`, `CheckedDialogueMarkHandler`, or
  `RuntimeDialogueMarkHandler`;
- access Select `propagates_error`, runtime `mark_handlers`, or unsafe
  `id_ref_label`;
- clone/publish `StatementScrutineeTypeAuthority` or a prepared Event/mark proof;
- pass a source String/PublicId/raw ExprId as semantic mark/unsafe/Trigger
  identity.

Positive compile fixtures prove public read-only accessors are sufficient for
compiler/verifier consumers.

## Codec, behavior, and dependency gates

- purpose-built transcript golden bytes cover all payload/role tags, mark
  coordinate bytes, ingress type outer tag `88` and inner tags `0..2`, and
  unsafe semantic IDs;
- mutation/property tests cover every field and exclusion in
  `MARK_COORDINATE_AND_TRANSCRIPT.md`;
- runtime-plan/AWBC tests prove existing typed runtime mark IDs and no public
  wire/version change;
- structured `cargo metadata`/architecture tests enforce dependency direction
  and Sans-I/O boundaries;
- AST-based inventory tests prove all 35 HIR variants and no wildcard producer;
- deletion compile tests prove old APIs unavailable; source searches supplement
  but do not replace these gates.

## Limit protocol

Tests inject small private limits so exact N and N+1 are practical. Production
uses the same checked-accounting code with accepted project/catalog bounds.
Every queue push, edge visit, catalog row, branch row, mark row, transcript
byte, and diagnostic count is charged before allocation/write. Arithmetic
overflow is a typed failure. An error publishes no registered environment,
final analysis, runtime semantic facts, transcript catalog, or `CheckedMatch`.
