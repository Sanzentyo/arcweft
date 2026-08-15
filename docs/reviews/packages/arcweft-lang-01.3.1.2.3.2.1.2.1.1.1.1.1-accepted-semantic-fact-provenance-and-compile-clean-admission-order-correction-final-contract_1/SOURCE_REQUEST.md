# Lang-01.3.1.2.3.2.1.2.1.1.1.1.1 — accepted semantic-fact provenance and compile-clean admission-order correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.2.1.1.1.1.1. It is a narrow mandatory
correction to the returned
Lang-01.3.1.2.3.2.1.2.1.1.1.1 external-lowering and independent
generation-admission contract.

The returned archive is retained as
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1.1.1-external-lowering-and-independent-generation-admission-authority-correction-final-contract.zip`,
SHA-256
`b1bd2e49c2d9a7859e12a22d85971aa9f9ad092600e05a2dc55c144d63c5056d`.
Its extracted mirror is the sibling package directory with the same canonical
basename. The download's leading `1-` was a delivery collision prefix and is
not part of the repository name.

The implementation audit used Git commit
`b1bf20910643206b75a315aba70f5ec468c03612` on `main`, equal to
`origin/main`, with an initially clean working tree before ZIP intake.

This is a design-only request. It must not return production code, a patch, an
overlay, or a compatibility path. Every Arcweft-owned schema, ABI, codec,
digest-domain, protocol, and persistence version remains exactly `1`.

## Accepted decisions that remain fixed

Do not redesign the following returned decisions without a concrete
current-source defect:

- public checked core-owned raw construction APIs for the legitimate
  `arcweft-runtime-plan` lowerer, with private fields, checked builders, and
  custom version-1 decode through the same validation path;
- raw `RuntimePlan` and `AwbcProgram` values are data, not operational
  authority, and have no self-admission methods;
- generation admission precedes raw plan/AWBC admission, and plan/AWBC
  admission requires the same exact issued parent;
- core stores only lower-layer owned generation facts and never depends on
  `arcweft-runtime-plan` or compiler types;
- atomic owner-only generic validation of `RuntimeCheckedType::Opaque`;
- effect-owned `AwbcTypedSite::AudioCommand { effect, slot }`, all 35 closed
  slots, no command-only coordinate, and no
  `AwbcEffectPlanTypedSlot::AudioValue`;
- checked versus closed operational plan type declarations, with root `[0]`
  and a fact for every present expression node;
- direct coordinate equality for plan/AWBC correlation, without another
  digest or caller-supplied root map;
- generation-first bundle/load/restore order, exact-parent hot swap, and the
  selected version-1 section tags; and
- the corrected Option `None`, opaque, AudioCommand, operational-expression,
  and raw-construction test semantics.

The accepted lower role vocabulary, Character/View canonical digests,
canonical `RuntimeValuePath`, `RuntimeCheckedTypePath`, outer value shapes, and
lossless project/producer root projections already on current `main` also
remain fixed.

## Residual blockers

### 1. The normative phase order cannot compile

P02 requires `arcweft-runtime-plan::final_expr` and `final_pattern` to emit
`RuntimeTypedExpr` and `RuntimeTypedPattern`. Every fact requires a
`RuntimePlanTypeId`, but that ID has no public constructor and is issued only
by `RuntimePlanBuilder::push_type`, whose owner is added in P03. P02 therefore
cannot complete or compile as specified.

P08 adds `AdmittedRuntimeProduct::checked_value_context` and
`nominal_record_domain`, but `AdmittedRuntimeProduct` is not introduced until
P10. This is another final-owner dependency inversion, not a validation
failure that can be deferred within the stated phase.

### 2. The actual lowerer has no complete accepted expression-type input

The return says `final_expr` receives an accepted semantic fact for every
expression node. Current `RuntimePlanSemanticFacts` has selected literal,
value, call, select, nominal, and dialogue facts, but no exhaustive
`ExprId -> accepted normalized type` table. `FinalExprLowerer` receives the
HIR module plus those runtime-plan facts. The complete expression types exist
earlier in `FinalSemanticAnalysis::CheckedExpression` and are not preserved by
the current projection.

Consequently the lowerer cannot mechanically choose the exact semantic
identity and `Checked` versus `Operational(RuntimeOperationalType)` row for
every root and child. Reconstructing it from `RuntimeExpr`, `RuntimeValue`, a
name, or a root-shape tag would lose normalized nested type identity and would
make admission circular.

### 3. The generation provenance claim is stronger than its API

All generation projection row constructors, the projection builder, the
canonical fact-section decoder, and
`AdmittedRuntimeGeneration::try_issue(projection)` are public. The aggregate
is non-Serde and has no conversion from a raw artifact, but any Rust caller can
still copy arbitrary internally consistent declarations into the public rows
and issue an admitted parent. Absence of `From<RuntimePlan>`, Serde, or public
fields is validation hygiene; it is not accepted-world provenance or a
non-forgeable capability.

The return simultaneously calls the issued parent independent/non-forgeable
and requires operational APIs to rely on it. It must select and state the real
trust boundary. Either the public issuer is deliberately a structural
validation API for trusted integrators, in which case the authority names,
claims, threat model, bundle-loader guarantees, and tests must say so, or a
genuine accepted-world issuance capability must be defined with a
layer-correct path from checked compiler facts and verified persisted facts.
The latter may not rely on caller name, crate path, feature flags, source text,
or an allegedly private workspace convention.

### 4. Three required construction surfaces are absent

`RuntimeNominalRecordFieldProjection` has private fields but no public checked
constructor or accessors. The external compiler assembly therefore cannot
construct the field rows required by
`RuntimeNominalRecordProjection::try_new`; this repeats the friend-crate
visibility defect at a narrower boundary.

`RuntimePatternBindingCoordinate` is referenced by `RuntimePatternBindingFact`
and the typed-site material but is never defined in the package or current
source. Its binding-family, local/path, whole/rest, identity, ordering, wire,
and error choices affect observable pattern binding and cannot be inferred
from the name.

The retained contract requires an AWBC `nominal_record_domains` table and
domain operands on `MakeRecord` and nominal record constants. The returned
`AwbcProgramBuilder` has no `push_nominal_record_domain`, the program has no
corresponding accessor, and the private wire/bounds/canonical-order behavior is
not specified. `AwbcNominalRecordDomainId` appears only at use sites, leaving
the domain table impossible to author or decode.

### 5. Synthetic expression nodes also need accepted types

Some final runtime expressions are synthesized during projection and do not
have their own source `ExprId`, including reduction/agent scaffolding,
assignment scaffolding, and synthesized empty/composite values. An exhaustive
`ExprId -> type` table alone cannot type these nodes. The contract must identify
the accepted source fact and deterministic derivation for every synthetic
node, or give it an explicit typed synthetic coordinate issued while lowering.
Runtime-value inspection is not an accepted semantic-type source.

## Required exact decisions

1. Define the one exhaustive accepted expression-type fact owner. Give its
   exact Rust-shaped row/key/value types, constructors, visibility, duplicate
   and completeness checks, snapshot/world correlation, and source error
   mapping from `FinalSemanticAnalysis` into `RuntimePlanSemanticFacts` or a
   selected replacement.
2. Define how `FinalExprLowerer`, final pattern/flow lowering, and generation
   assembly borrow that same accepted fact without copying a second type map.
   Give the exact projection from every current normalized semantic type to
   `(RuntimeSemanticTypeId, RuntimePlanTypeKind)`, including all closed checked
   and operational families and unsupported/unknown errors.
3. Define the exact interning/build sequence that makes
   `RuntimePlanTypeId` available before a typed expression or pattern is
   constructed. Select the owner of the mutable type interner/builder, the
   lifetime of returned IDs, canonical duplicate behavior, atomic `finish`,
   and the external lowerer's exact call order. Do not add an unchecked public
   ID constructor.
4. Select the actual trust and provenance model of
   `AdmittedRuntimeGeneration`. If public projection plus `try_issue` is a
   trusted-integrator structural boundary, state that explicitly, remove
   non-forgeability claims that the type cannot enforce, and define which
   operational entry points additionally require compiler-owned or
   verified-bundle evidence. If public callers must not mint accepted-world
   authority, define the exact opaque issuance input/token, its legitimate
   compiler and bundle constructors, dependency direction, lifetime, and why
   arbitrary Rust callers cannot construct or replay it.
5. Reconcile canonical persisted generation facts with decision 4. Specify
   whether a verified version-1 bundle fact section is itself an accepted
   authority, what container/signature/trust checks issue that status, and why
   the public fact decoder alone cannot publish an operational generation.
   No raw plan/AWBC declaration may fill or amend missing accepted facts.
6. Give final APIs for compiler assembly, bundle loading,
   `try_admit_plan`, `try_admit_awbc`, pair admission, runtime-driver
   publication, restore/replay, and direct VM/AOT execution under the selected
   trust model. Identify every public convenience that can publish an
   operational object and the exact evidence it consumes.
7. Replace the implementation order with compile-clean stages. In particular,
   land the type-ID issuer before migrating the external lowerer, and land
   `AdmittedRuntimeProduct` before inherent product context/domain methods.
   Each stage must name additions, consumer migrations, same-stage deletions,
   and focused compile/test commands.
8. Regenerate the normative test matrix and producer/consumer/deletion
   inventory for the selected semantic-fact carrier, generation provenance,
   builder order, bundle trust boundary, product context order, and all
   affected compiler/runtime-plan/core/bundle/driver/VM/AOT/restore consumers.
9. Complete the public checked projection surface for nominal record fields.
   Give the exact `RuntimeNominalRecordFieldProjection` constructor,
   validation, field/type accessors, order/duplicate rules, and compiler call
   site. Do not expose writable fields or an unchecked row constructor.
10. Define `RuntimePatternBindingCoordinate` completely: owner module, closed
    variants, local/binding identity, whole/rest/path behavior, canonical
    ordering and version-1 wire tags, checked constructors, accessors, error
    mapping, plan-site relation, lowering, decode, and tests.
11. Restore the one AWBC nominal-record-domain table to the final aggregate
    contract. Specify its row type, ID issuer, builder push method, program
    accessor, canonical order/duplicates/limits, private wire grammar, exact
    `MakeRecord` and nominal constant operands, verifier/VM resolution, and
    project-versus-producer domain correlation. Do not create a second domain
    map or derive a domain from a nominal spelling.
12. Give an exhaustive table for source-backed and synthetic runtime
    expression nodes. For every synthetic node, name the accepted semantic
    fact or typed construction rule that supplies its semantic identity and
    checked/operational kind, plus deterministic missing/mismatch errors.

## Required tests

The returned design must require executable evidence that:

- every present `RuntimeExpr` root and child obtains its exact accepted
  semantic identity and checked/operational kind from the retained semantic
  analysis fact, never from a reconstructed runtime value or spelling;
- missing, duplicate, stale-snapshot, wrong-world, unsupported, and
  semantically mismatched expression facts fail at deterministic typed owners;
- the real external lowerer interns a type before constructing facts that use
  its `RuntimePlanTypeId`, and no unchecked ID construction path exists;
- every numbered phase compiles with only final owners available;
- the selected generation trust model is tested at every public publication
  boundary, including direct public Rust calls, decoded fact sections,
  compiler output, bundle load, raw-artifact tamper, restore, replay, and hot
  swap;
- raw plan/AWBC declarations cannot create or modify the independent facts
  against which they are admitted; and
- checked-value contexts and nominal domains cannot be requested before one
  admitted same-parent product exists;
- the compiler constructs nominal field projections through the final public
  checked API, while field literals and unchecked rows fail to compile;
- every pattern binding family round-trips through one closed
  `RuntimePatternBindingCoordinate` grammar and resolves to its exact plan
  binding owner;
- AWBC nominal record domain rows are authored, encoded, decoded, admitted,
  and consumed through one table and cannot be omitted or replaced by a
  spelling-derived domain; and
- every synthetic expression node has an exact accepted type source and
  missing or mismatched synthetic evidence fails deterministically.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-and-compile-clean-admission-order-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, the exact current request copy/hash,
current-main Git/source evidence, exact Rust-shaped APIs, corrected phase and
fact-flow tables, a consistent test matrix, complete inventory, and no
production patch or overlay. Keep every sidecar inside the ZIP.
