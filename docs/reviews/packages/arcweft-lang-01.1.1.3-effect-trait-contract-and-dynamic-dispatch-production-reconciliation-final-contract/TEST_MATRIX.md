# Test matrix

Every row is mandatory unless marked `non-applicable` by the E017 supersession. Tests must use typed APIs, behavior, diagnostics, compile-fail visibility, or structured dependency evidence. Repository source-text scanning is prohibited.

| ID | Layer | Polarity | Case | Required result | Focused gate |
|---|---|---|---|---|---|
| S001 | syntax | positive | trait method explicit effects clause | one source-backed ContractClause; exact whole/keyword/item spans | focused syntax |
| S002 | syntax | positive | trait method effects {} | closed empty authored row; exact empty payload span | focused syntax |
| S003 | syntax | positive | impl method multiple effects clauses | one unioned contract; source order retained | focused syntax |
| S004 | syntax | positive | ordinary/flow/trait/impl clause parser parity | same ContractClauseKind and source model | focused syntax |
| S005 | syntax | negative | malformed trait effect selector | ordinary contract parse recovery; no sema reparse | focused syntax |
| S006 | syntax | negative | shadow parser event for effects clause | same clause classification; no subset reader | focused syntax |
| S007 | source | positive | method name/signature/body ranges | revision-bound spans exactly match authored bytes | syntax+HIR |
| S008 | source | positive | multi-byte UTF-8 before method/row | all byte ranges validate and LSP converts correctly | syntax+LSP |
| I001 | HIR identity | positive | trait requirement ID | package/module/trait/method key and exact context | focused HIR |
| I002 | HIR identity | positive | trait impl method ID | package/module/impl ordinal/kind/method key | focused HIR |
| I003 | HIR identity | positive | inherent method ID | kind differs from trait impl under same impl owner | focused HIR |
| I004 | identity | positive | two same-named traits in different modules | distinct TraitDeclarationId and method IDs | sema |
| I005 | identity | negative | duplicate same-named traits in one module | typed duplicate; no accepted duplicate callable record | sema |
| I006 | identity | positive | trait import alias | lookup binding targets original requirement ID | HIR+sema |
| I007 | identity | positive | trait reexport chain | downstream lookup preserves original ID and row | project |
| I008 | identity | positive | method reached through alias | target facts use original requirement/impl ID | sema |
| I009 | identity | negative | private trait alias | visibility diagnostic; zero method candidate/effect edge | project+sema |
| I010 | identity | negative | missing trait identity | typed missing; no spelling fallback | sema |
| I011 | identity | negative | ambiguous imported traits | all typed candidates ordered; no first-match row | sema |
| I012 | identity | negative | stale ProjectSymbolRevision ID | CheckedCallableCatalog lookup rejects stale | sema API |
| I013 | identity | negative | stale detached SourceDocumentIdentity | lookup rejects wrong revision | sema API |
| I014 | identity | positive | rollback registered method/call edge | checkpoint rollback leaves no record/edge/row leak | sema transaction |
| I015 | identity | positive | rollback then committed recheck | same exact source ID is valid only in committed catalog | sema transaction |
| I016 | identity | positive | standard trait method version | exact StandardTraitCatalogVersion(1) plus append-order StandardCallableDeclarationId | sema |
| I017 | identity | positive | detached source-bound callable ordinal | unified source-order key plus exact SourceDocumentIdentity; no fabricated package | HIR+sema |
| I018 | identity | negative | source-less detached method catalog | typed SemanticSourceUnavailable before checked catalog construction | HIR+sema |
| E014 | traits/effects | positive | bodyless requirement explicitly permits control.suspend | dispatch exposed row contains control.suspend | sema |
| A023 | traits/effects | positive | awaiting impl for E014 | actual row contains control.suspend; conformance accepted | sema+compiler |
| E015 | traits/effects | negative | omitted bodyless row and awaiting impl | closed empty; exact TraitOmittedRowMissing payload/ranges | sema diagnostics |
| E016 | traits/effects | negative | explicit row excludes suspension | TraitClosedRowMissing with deterministic shortest trace | sema diagnostics |
| E022 | effects | negative | direct Await under effects {} | ClosedRowMissing; row primary, Await keyword related | sema diagnostics |
| E023 | effects | negative | transitive callee reaches Await | same code; shortest typed call path to Await | sema diagnostics |
| E024 | effects | positive | open typed row tail receives suspend | tail absorbs residual and retains all prior effects | effect row |
| E025 | effects | positive | multiple actual effects subset of broad row | accepted; no effect lost | effect row |
| E026 | effects | negative | multiple residual effects against closed row | one diagnostic with causes sorted by EffectId | diagnostics |
| E027 | effects | positive | authored unused permitted effect | accepted; exposed row remains authored contract | sema |
| E028 | effects | negative | unknown permitted tail | typed unresolved/unknown-row rejection before lowering | sema |
| E029 | effects | negative | unresolved actual tail against closed row | not treated empty; typed blocking error | effect row |
| E030 | effects | positive | existing typed tail plus effects clauses | concrete head union plus existing open tail | sema |
| E031 | effects | positive | NoEffect with unbounded body | forbidden constraint remains distinct from upper bound | sema |
| E032 | effects | negative | own row and requirement row both violated | two owner-distinct typed diagnostics, deterministic order | diagnostics |
| T001 | traits | positive | inherited requirement | original declaring requirement ID retained | sema |
| T002 | traits | positive | one impl satisfies compatible same-name inherited requirements | one conformance per original requirement | sema |
| T003 | traits | negative | incompatible inherited signatures | typed inherited conflict before effect conformance | sema |
| T004 | traits | negative | compatible signatures but one narrower row | only violated requirement diagnosed | sema |
| T005 | traits | positive | generic Self/type substitution | actual and permitted rows checked after typed substitution | sema |
| T006 | traits | positive | associated type substitution | same requirement ID; correct substituted signature/row | sema |
| T007 | traits | positive | effect variable already partially bound | residual merged, not overwritten | effect row |
| T008 | traits | negative | effect variable existing binding rejects residual | sorted missing effects and trace | effect row+diagnostics |
| T009 | traits | positive | body-bearing inherent method without row | ordinary one-pass inference; no closed-empty default | sema |
| T010 | traits | negative | authored bodyless impl method | typed implementation error; no empty inferred record | sema |
| T010S | traits | positive | programmatic standard bodyless implementation | ExternalOrStandard installed row; no body inference or fake source | sema |
| T011 | traits | negative | default trait body remains deferred | existing typed support-boundary rejection | sema |
| R001 | resolver | positive | concrete trait method call | candidate target is impl CheckedCallableId and conformance | sema |
| R002 | resolver | positive | inherent method call | candidate target is inherent CheckedCallableId | sema |
| R003 | resolver | positive | static witness call | requirement ID + witness; requirement exposed row | sema |
| R004 | resolver | positive | method value concrete trait | BoundMethodValue captures receiver once and impl target | sema+compiler |
| R005 | resolver | positive | method value static witness | requirement ID/witness/substituted latent row | sema+compiler |
| R006 | resolver | positive | curried method non-final group | only argument evaluation effects; same target retained | sema |
| R007 | resolver | positive | curried method final group | latent exposed row applied once | sema+compiler |
| R008 | resolver | positive | method value with multiple remaining groups | signature/groups/result/effects preserved exactly | sema |
| R009 | resolver | negative | ambiguous method value | typed ambiguity; no BoundMethodValue/effect edge | sema |
| R010 | resolver | negative | private method value | visibility diagnostic; no value target | sema |
| R011 | resolver | positive | direct call and bound final call parity | same lowering target and effect row | compiler |
| R012 | resolver | positive | work accounting | one resolver invocation, one target fact, no legacy dispatch | sema counters |
| R013 | resolver | negative | unknown indirect function row | reject before lowering; never assume pure | sema+compiler |
| P001 | compiler/runtime identity | positive | trait impl method projection | implementation RuntimeCallableId equals exact checked-ID digest projection | core+compiler |
| P002 | compiler/runtime identity | positive | static witness projection | implementation and original requirement projections retained together | compiler+runtime plan |
| P003 | compiler/runtime identity | positive | two same-named traits in different modules | distinct checked digests and runtime callable projections | HIR+sema+compiler |
| P004 | compiler/runtime identity | positive | deterministic lower input order | same checked inputs produce identical RuntimeTraitMethodId assignment | compiler |
| P005 | compiler/runtime identity | positive | typed conformance lowering index | conformance maps directly to emitted method ID; no method-name lookup | compiler |
| P006 | compiler/runtime identity | positive | inherent method lowering index | checked inherent method ID maps directly to emitted method ID | compiler |
| P007 | compiler/runtime identity | positive | runtime inventory serialization round trip | two-field identity and method vector round trip exactly | core+runtime plan |
| P008 | compiler/runtime identity | negative | stale checked context before projection | projection request rejected with stale identity; no runtime method emitted | sema+compiler |
| P009 | compiler/runtime identity | positive | iterator witness evidence | ForIterationEvidence carries exact method conformance IDs and emits direct into_iter/next RuntimeTraitMethodIds | sema+compiler+runtime plan |
| P010 | compiler/runtime identity | positive | runtime has no effect authority | execution uses direct method ID; row/source resolution remains absent | runtime plan behavior |
| D001 | diagnostics | positive | direct effect trace | zero call steps, exact terminal span | sema |
| D002 | diagnostics | positive | two equal-length paths | lexicographic typed source/ID tie-break | sema |
| D003 | diagnostics | positive | recursive call cycle | finite deterministic shortest path | sema |
| D004 | diagnostics | positive | two missing effects with shared call span | one diagnostic, coalesced label, sorted effects | sema |
| D005 | diagnostics | positive | multiple authored effects clauses | first primary, later related in source order | sema |
| D006 | diagnostics | positive | E015 omitted row primary | exact requirement method-name span | sema |
| D007 | diagnostics | positive | CLI typed projection | code/message/primary/related payload exact | CLI |
| D008 | diagnostics | positive | LSP typed projection | same diagnostic and revision-validated protocol ranges | LSP |
| D009 | diagnostics | negative | stale LSP snapshot | stale report discarded; no fallback ranges | LSP |
| D010 | tooling | positive | hover on trait method | original requirement/impl ID and exposed row | LSP |
| D011 | tooling | positive | signature help on bound method curry | receiver removed; groups/effect row exact | LSP |
| D012 | project index | positive | trait requirement index record | navigation/signature/effect query; not runtime target | project index |
| D013 | project index | positive | impl/inherent method index records | checked IDs and catalog delegation, no row copy | project index |
| E017 | dynamic traits | non-applicable | parent dynamic trait object row | status SUPERSEDED_FOR_LANG_01_1_1; never counted by static witness | contract audit |
| E017S | static witness | positive | static witness effect dispatch | requirement exposed row after substitution across sema/compiler/tooling | sema+compiler+LSP |
| X001 | syntax | negative | dyn trait object type spelling | ordinary grammar rejection; no executable typed object | syntax+sema |
| X002 | API removal | compile-fail | construct dynamic trait-object public type/target | no such public typed variant/API | trybuild |
| Z001 | API removal | compile-fail | import TraitCallableId | type removed; checked ID is sole target identity | trybuild |
| Z002 | API removal | compile-fail | construct legacy source CallableId from String | legacy source-callable constructor unavailable | trybuild |
| Z003 | API removal | compile-fail | construct UpperBoundExceeded diagnostic | variant removed | trybuild |
| Z004 | API removal | compile-fail | construct Project effect schema with declared row | row-copy field removed | trybuild |
| Z005 | behavior removal | positive | resolved project method value | succeeds through BoundMethodValue, not old rejection | sema |
| Z006 | behavior removal | positive | trait method effect lookup | catalog query returns authored/inferred row; no hard-coded empty | sema |
| Z007 | structured dependency | positive | crate dependency direction | syntax -> HIR -> sema -> compiler/tooling; no reverse edge | cargo metadata |
| Z008 | structural | positive | owner enum behavior | new variants handled by inherent impl/API tests | unit |
| Z009 | structural | positive | source-gate absence | removal evidence consists only of API/behavior/dependency tests | test review |
| Z010 | API removal | compile-fail | construct old RuntimeTraitMethodIdentity fields | local indices and trait/method/self-type/monomorph strings are unavailable | trybuild |
| Z011 | API removal | compile-fail | access RuntimeTraitMethodInventory.by_witness_method | string/index lookup map is removed | trybuild |
| Z012 | behavior removal | positive | runtime evidence construction | accepts direct typed lowering-index result, not witness plus method string | compiler |

## Required focused suites

- `arcweft-lang-syntax`: source-backed contract clauses and trait/impl method ranges.
- `arcweft-lang-hir`: project/revision-bound trait, impl, and method declaration publication.
- `arcweft-lang-sema`: catalog ownership, rows, conformance, resolver, method values, fixed point, diagnostics, rollback.
- compiler: direct/bound/static-witness target, checked-ID runtime projection, typed lowering index, and curry lowering parity.
- `arcweft-core` / `arcweft-runtime-plan`: two-field trait-method identity, vector-only inventory, direct runtime IDs, and serialization round trip.
- project semantic index/CLI: checked-catalog projection and diagnostics.
- LSP: hover, signature, diagnostics, UTF-8/revision range projection.
- compile-fail (`trybuild` or repository-standard equivalent): removed public/crate-owned APIs.

## Broad gates

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
just test-tier2
```

`just test-tier2` is applicable because the final switch spans multiple crates, materially changes a public checked-callable/project-index contract, and reaches the project/Agent semantic-index path. It must not preserve an obsolete shape merely to satisfy a stale slow test.

## Exact removal-evidence rule

No test may open repository implementation/documentation files and search for `TraitCallableId`, `AWF-EFX-001`, `by_witness_method`, old runtime identity fields, helper names, source paths, enum variants, or spellings. Compile-fail tests compile consumer code against the API; behavior tests resolve/check/lower calls; serialization tests use public typed values; structured tests inspect Cargo metadata or typed registry contents.
