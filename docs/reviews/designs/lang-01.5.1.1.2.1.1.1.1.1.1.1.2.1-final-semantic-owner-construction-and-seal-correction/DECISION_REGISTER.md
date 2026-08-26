# Decision register

| ID | Decision | Selected result | Rejected alternatives |
|---|---|---|---|
| D1 | publication | `FinalSemanticAnalysis` owns sealed entries and nominal projections | public builder, wrapper publication, compiler side table |
| D2 | phases | private draft -> Entry check -> EntryRef seal -> final validation/publish | post-publication patch, second analysis |
| D2a | prepared facts | private consumable expression/pattern typestate rows for Complete, Entry, and projection-dependent C2 seeds; one shared candidate journal for expressions | pending side map, final-row placeholder, public pending variant |
| D3 | verification precedence | Entry binding precedes verification; Entry selection remains after | exposing draft to verifier, reverse dependency |
| D4 | Entry checker input | concrete narrow prepared authority | `&FinalSemanticAnalysis`, public trait |
| D5 | nominal projection | one exhaustive typed request visitor/context over symbols + accepted type map, sealed complete catalog retaining `TypeShape` | demand-only seeds, duplicate projector, reader recomputation |
| D5a | nominal construction order | C2.2a context foundation -> C2.2b Record authority -> C2.3 exact final types plus consumable seeds -> C2.4 exhaustive visitor, digest-order projection, seed consumption, and catalog seal | source-order projection, visitor over placeholder rows, partial inventory |
| D5b | compile scaffold | existing final-analysis projection wrappers temporarily delegate in C2.2a, remain uncommitted/unpublished, and are deleted in C2.4 | separate accepted cut, retained wrapper authority, intermediate commit/push |
| D5c | projection precedence | Analyzer moves a draft into disjoint parts; one borrowed post-Analyzer context projects the complete inventory in `SemanticTypeDigest` order before cached-only row/Entry seals | long-lived self-borrow, `Arc` type-map sharing, per-producer context, source-order first error |
| D6 | nominal limits | fresh `NominalResolutionLimits` budget per root plus non-resetting `NominalAggregationLimits` project budget | one global root budget, new constants, saturation |
| D7 | environment fields | ordered typed Record semantics inside existing accepted nominal record/catalog/world digest; typed owner/ordinal failure at executable lowering | TypeCheckEnv map/index, public raw record/field mint, diagnostic-name reconstruction |
| D8 | environment patterns | admit exact accepted named record using same rows | reader name reconstruction |
| D9 | View modifier | delete success and fail closed | invented registry, name hash |
| D10 | shared types | reuse `DeclarationIdentityFamily`, `CallableReceiverMode`, one field-ID enum | parallel enums |
| D11 | variants | one owner table plus selected ordinal/borrowed accessor | cloned selected row |
| D12 | Character look | hash exact accepted manifest Character/look/selection row | HirName fallback, ID-only unvalidated row |
| D13 | Style | owner-defined exhaustive 26-variant encoder | literal-count gate, Serde/debug |
| D14 | Postfix | selected ExprId remains private lookup-only; C3 hashes child digest | deleting live lookup, hashing ID |
| D15 | RichText | C2 retains typed report; C3 derives token/open ordinals and digest | C2 partial digest, raw tag ID hash |
| D16 | dead Select | delete TupleElement and RecordElement; reserve `0x0405/0x0406` | fabricate producers, tag reuse |
| D17 | versions | every new domain remains version 1 | V2, compatibility path |
| D18 | C1 | consume unchanged | topology/path redesign |
| D19 | callable join handoff | compose one private call-owner join map after callable/call finalization, enrich Method from it, then move the same joins into edge facts | call-resolution-time digest, edge-time recomputation, final side map |
| D20 | runtime field handoff | project rows expose typed runtime field coordinates; record expression rows move into child-edge facts and record pattern rows lower directly | restoring `CheckedProjectNominal`, source-name lookup, fabricated environment runtime layout |
| D21 | call publication | one private prepared transaction, then an acyclic post-callable/effect core/continuation/final `CheckedCallApplication` seal; only the complete application publishes, and selected execution and unselected diagnostics are distinct | provisional public fact plus pending duplicate, final rebuild, execution API on recovery |
| D22 | call typing | one candidate-wide opaque lower normalized-binding solution under an exact rigid/bindable/future-eligible scope over inherited continuation, instantiation, receiver, source-ordered arguments, and expected result; callable alone owns deferred group rows and final pair uniqueness | lower callable-group/deferred ownership, join inference, per-equation runs, caller-side binding merge, binding-only MGU uniqueness, optional `T`/`Option<T>` search |
| D23 | argument meaning | mapping-owned source projection composed with schema-owned typed value alternatives | one enum bag for declared/clear/rest/unchecked/unmapped, source spelling tests |
| D24 | intrinsic generics | lower exhaustive language-intrinsic generic owners and closed terminal solutions | Option-only special case, `Named("_")`, unchecked/context-rebuilt schema, implicit `Never` defaults |
| D25 | continuation | `CheckedCallResult::Continuation` is the only group/solution owner and creates curried candidates opaquely | raw base plus duplicated `next_group`, function-type schema reconstruction |
| D26 | execution lowering | sealed sema execution projection owns generation-local callee/receiver/argument sources while the selected candidate owns dispatch | common callee fact; compiler HIR/name/schema reconstruction of dispatch, sources, receiver, operands, actions, or partial shape |
| D27 | stable coordinates | move the C1 coordinate/path atom algebra and encoder into one sema-root lower owner shared by callable and final analysis | callable-to-final-analysis dependency, copied coordinate enum, raw local/function IDs in stable identity |
| D28 | constraint control | one candidate session exclusively borrows `ResolverWork`, reserves its previous/proposed full-report transition, and exposes only narrow charging; complete or Drop infallibly commits exactly once and releases the borrow before outcome access | value-consuming/raw resolver work in callbacks, caller-side report merge, fallible drop, raw-work return, pending getters, limits-only recursion, unchecked side counters |
| D29 | compatibility | one types-owned Recovery/SelectedCall/Invariant directional engine; Invariant rejects recovery/unresolved values while retaining canonical widening and rigid generics, structural mismatch is diagnostic-only and verdict-independent, and array length is policy-owned and exact outside recovery | structural Invariant, compatibility verdict from first mismatch, compatibility copies in checker/solver, standalone array wildcard helper, unmetered recovery acceptance during selected sealing |
| D30 | source-dependent constraints | callable-owned generic callbacks receive borrowed `ExpectedHint::{Unchecked, Complete, Parametric}` plus a narrow work session; isolated probes retain every correlated trace, then each trace is re-materialized from one baseline in exact source order and coalesced only by sealed final value | reverse source hints, optional single branch request, source facts shared across probes, binding-only branch deduplication, first-success contextual checking, argument-only selected replay |

All decisions are closed.

## D31 — accepted semantic root catalog

Accepted checked coordinates use one version-1 root grammar: declaration roots
carry tag `0x00`, item roots carry tag `0x01`, and both carry a 32-byte
catalog-issued digest. The project evaluation topology is built once per
Analyzer generation and leased by an `AcceptedSemanticRootCatalog`; item
family/role coordinates are typed and source-ordered. Match and producer
coordinate issuance consumes that catalog and does not accept a declaration
argument or rebuild declaration paths. The HIR sole admission constructor
validates the project module/snapshot and symbol generation join once. The root
catalog retains that accepted topology and joins it to the checked callable
catalog only by pointer identity of their shared generation token. Raw report
reuse is validated by structural readmission through the same HIR constructor;
the root catalog does not repeat project, module, snapshot, or symbol scans.

Erratum note (2026-08-25): D27's shared C1 value coordinate is the exact
two-case algebra `Expression(CheckedSemanticPath) |
Binding(StableCheckedBindingCoordinate)`. The binding coordinate owns one full
checked path; capture rows therefore use the binding coordinate and cannot
reconstruct a pattern binding or recursively wrap another value coordinate.
Path/byte lengths are canonical checked little-endian `u64`; ordinal payloads
remain `u32`. Production validation of this in-place correction is pending as
recorded in `VALIDATION_REPORT.md`.
