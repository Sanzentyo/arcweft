# Final contract

## 0. Scope and status

This design closes only the residual boundaries in the maintained request at
`35d42efdd89fef8fde73f62be2a3e38fd5e81e52`. `OPEN_QUESTIONS=0`. It is design-only and contains no production
patch. Every Arcweft-owned schema, ABI, codec, persistence, digest-domain, and
protocol marker remains exactly `1`.

## 1. One final RuntimePlan owner

`RuntimePlanBuilder` in `arcweft-core::plan::construction` is the sole mutable
aggregate owner. Its constructor **consumes** the already-canonical
`RuntimeLocalDeclarationTable`. It owns the existing non-cloneable
`RuntimePlanTypeTableBuilder`; it never copies local declarations or exposes a
second type interner. `RuntimePlan` fields become private, `Default` is removed,
and custom version-1 decode feeds the same builder methods used by the external
lowerer.

The existing type interner's first-seen semantics remain normative. The only
addition is an inherent `intern_batch`, used to make one typed expression or
pattern atomic: it preflights existing/intra-batch conflicts and capacity,
then commits in supplied canonical pre-order. Exact duplicates return the same
ID; conflicting kinds or exhaustion mutate nothing. `intern` delegates to a
single-row batch.

## 2. Typed expression/pattern transaction

`RuntimeIndexPath` is non-empty, begins with `0`, has at most 64 indices, and
uses checked `u32` ordinals. `RuntimeExpr::try_visit_nodes` and
`RuntimePattern::try_visit_nodes` are inherent exhaustive canonical pre-order
visitors on the original enums. They are structural enumerators, not type
resolvers.

The lowerer submits `(RuntimeIndexPath, RuntimePlanTypeDeclaration)` rows to the
builder. The builder requires exactly one row for every present node and no
extra/duplicate path, interns the entire batch atomically, validates all
pattern binding coordinates against its local table, and only then creates
`RuntimeTypedExpr` / `RuntimeTypedPattern`. Their fields stay private; raw node
access and type lookup are borrowed accessors only. A typed wrapper is owned by
one plan site; it cannot be detached into a sidecar authority.

## 3. Exact lowering order

For every HIR project generation:

1. final semantic analysis is validated against the exact final-HIR snapshots;
2. the existing HIR runtime owner inventory enumerates source owners and the
   closed synthetic site keys;
3. `RuntimePlanSemanticFacts::try_new` validates source and synthetic fact
   completeness, duplicates, owner family, snapshot, and world;
4. generation facts are projected while facts are still borrowed;
5. `into_lowering_parts` moves the canonical local table into one
   `RuntimePlanBuilder` and moves all accepted fact maps into a non-cloneable
   `RuntimePlanLoweringFacts`;
6. `FinalFlowLowerer` owns that builder; every `FinalExprLowerer` and
   `FinalPatternLowerer` borrows the same `&mut RuntimePlanBuilder`;
7. each lowerer creates a private raw draft and complete declaration list,
   then immediately asks the builder to atomically issue IDs and seal a typed
   wrapper before inserting it into any plan owner;
8. `RuntimePlanBuilder::finish` seals the type table and private plan;
9. the plan is admitted against the already-issued generation;
10. AWBC lowering consumes `&AdmittedRuntimePlan`, builds through one
    `AwbcProgramBuilder`, admits the program, then pairs the same-parent product.

No raw draft or untyped node crosses the `arcweft-runtime-plan` crate boundary.

## 4. Accepted type provenance

Source-backed expression roots use only `RuntimePlanSemanticFacts::expression_type`.
Source-backed patterns use only `pattern_type`; locals use only `local_type`.
Synthetic nodes use one explicit `HirRuntimeSyntheticExprSite` or
`HirRuntimeSyntheticPatternSite` row in the same accepted aggregate. The row
stores `RuntimeNormalizedType`, so `runtime_plan_type_kind` remains the sole
checked/operational classifier. The runtime-plan lowerer does not inspect a
`RuntimeValue`, source spelling, type name, crate path, or nominal string to
recover a semantic type.

The synthetic coordinate vocabulary and exact source for every current node is
normative in `SYNTHETIC_EXPR_TYPE_TABLE.csv` and
`SYNTHETIC_PATTERN_TYPE_TABLE.csv`. Missing, duplicate, stale-snapshot,
wrong-world, unsupported, and semantic mismatch errors are deterministic and
precede builder mutation.

## 5. Nominal field projection

`RuntimeNominalRecordFieldProjection::try_from_accepted_ordinal` is the only
public row constructor. It checked-adds one to a zero-based defining ordinal,
checks `u32`/nonzero conversion through the existing private accepted ID
constructor, and stores the exact accepted semantic type. Its fields are
private and its only public reads are `field()` and `ty()`. The aggregate
projection constructor validates exact defining order, count, duplicate/gap,
field ID, and accepted layout type. The compiler enumerates the accepted layout
and calls this API; it cannot use a field literal or unchecked ID constructor.

## 6. Generation facts and trust

Core root fact constructors accept semantic identities/owners and derive
`RuntimeProjectRootId` / `RuntimeProducerRootId` internally. No caller supplies
a root map or catalog digest. One canonical aggregate combines core facts with
facts returned by the existing Character, View, and CharacterDialogue catalog
owners.

`AdmittedRuntimeGeneration::try_issue` is a public structural issuance API.
A safe Rust caller invoking it is explicitly a **trusted integrator**. The
issued type guarantees canonical ordering, duplicate/limit checks, internal
root derivation, exact opaque/nominal correlations, and immutable parent
identity. It does not prove that the caller's semantic world came from the
Arcweft compiler. Private fields and non-Serde status are hygiene only.

`CompilerRuntimeEvidence` adds official compiler-path provenance and binds the
final-HIR snapshot set, semantic owner transcript, generation identity, raw
plan digest, and raw AWBC digest. `VerifiedRuntimeBundleProduct` adds exact
container bytes, section uniqueness/order, canonical decode, independent fact
validation, plan/AWBC admission, same-parent pairing, and trust-policy result.
With `RequireTrustedEd25519`, it additionally authenticates a configured trusted
key; signature presence alone is not trust. With `TrustedIntegrator`, the host
is explicitly accepting the integrator boundary.

## 7. Admission product

Raw plans/programs have no admission methods. Free consuming functions require
an independent `Arc<AdmittedRuntimeGeneration>`. Plan admission creates a
private `Arc<RuntimePlanAdmissionKey>`. AWBC admission stores the exact same
parent Arc and an exact clone of that plan key after direct typed-site/domain
correlation. Pairing requires `Arc::ptr_eq` on both parent and plan key.

Only the paired `AdmittedRuntimeProduct` issues `RuntimeCheckedValueContext`
and `RuntimeNominalRecordAdmissionDomain`. Raw `RuntimePlan`, raw
`AwbcProgram`, plan-only admission, and AWBC-only admission cannot issue either.
The context validates a `RuntimeValue` against an already-selected checked type;
it never reconstructs semantic type from the value.

## 8. AWBC nominal-record domain

`AwbcProgramBuilder` is the sole issuer. Lowering first interns opaque staging
handles. Before finish it rejects same-origin/different-type conflicts, sorts
unique rows by their exact v1 encoded origin bytes plus type ID, assigns
zero-based contiguous final IDs, builds one ephemeral handle-to-ID remap, and
atomically rewrites every record instruction/constant draft. The remap is then
dropped. `AwbcProgram` exposes only a borrowed table accessor.

`MakeRecord` keeps opcode `0x0f`; record constants keep tag `12`. Both replace
`ty + field_names` with a closed construction operand: structural records carry
`ty + field_names`, while nominal records carry mandatory
`AwbcNominalRecordDomainId` and defining-order values. Project and producer
origins are distinct domain variants. The verifier resolves the domain before
arity/type checks; the VM consumes the admitted resolved row and never derives a
domain from a spelling.

## 9. Publication

`arcweft-runtime-driver` publishes only a `VerifiedRuntimeBundleProduct` under
an explicit `RuntimePublicationPolicy`. The compiler's direct-run path encodes
an in-memory version-1 bundle and invokes the same bundle verifier; it is not a
second publication route. Host bindings contain adapters/capabilities only,
never roots or catalog digests.

VM execution uses the core AWBC VM through `PublishedRuntimeGeneration`.
Cranelift uses `arcweft-lang-jit-cranelift`; AOT/native generation uses
`arcweft-runtime-codegen`. Hot swap accepts a new verified product, performs all
checks without mutation, and commits atomically only when the exact generation
parent is permitted. Restore/replay order is: verify bundle, admit generation,
admit plan, admit AWBC, pair, publish, then decode snapshot/value/event bytes
through the product-issued context.

## 10. Raw-artifact non-authority

Private raw fields, checked builders, custom v1 decode, absence of self-admission,
independent parent input, and consuming admission jointly ensure raw artifacts
cannot add, remove, replace, or self-approve accepted facts. A decoded fact
section is quarantine data until the bundle verifier constructs the independent
canonical aggregate and issues evidence. No fallback resolver, parallel type
map, compatibility alias, old reader, V2/V3 type, or source reconstruction is
permitted.
