# Lang-01.3.1.2.3.2.1.2.1.1.1.1 — external lowering and independent generation-admission authority correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.2.1.1.1.1. It is a narrow mandatory correction
to the returned Lang-01.3.1.2.3.2.1.2.1.1.1 checked-value path and resolvable
root-site contract.

The returned archive is retained as
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1.1-checked-value-path-and-resolvable-root-site-correction-final-contract.zip`,
SHA-256
`b8a2d1d5e09ad21c5372af11454f3f22188046d3af6bafc4637e0446c2cd531b`.
Its extracted mirror is the sibling package directory with the same canonical
basename. The download's leading `1-` was a delivery collision prefix and is
not part of the repository name.

The implementation audit used Git commit
`eb450570acff118ccc3e2a75751144f037af170f` on `main`, equal to
`origin/main`, with an initially clean working tree before ZIP intake.

This is a design-only request. It must not return production code, a patch, an
overlay, or a compatibility path. Every Arcweft-owned schema, ABI, codec,
digest-domain, protocol, and persistence version remains exactly `1`.

## Accepted decisions that remain fixed

Do not redesign the following accepted and validated substrate without a
concrete current-source defect:

- the lower `CharacterDialogueRuntimeRole` owner and inherent vocabulary;
- the canonical CharacterCatalog and ViewRegistry digest owners/transcripts;
- one canonical Serde `RuntimeValuePath` in `value::ownership::path`, with
  `OpaquePayload` as tag `10`, plus a distinct non-Serde
  `RuntimeCheckedTypePath`;
- the exact checked path push rules for Sequence, Tuple, Choice, Result,
  Option, Variant, nominal fields, and physical byte sequences;
- the complete current `RuntimeValueShape` table and descriptor-sourced
  nominal semantic-identity comparison;
- lossless 32-byte projection from `RuntimeSemanticTypeId` to distinct
  project/producer root newtypes;
- exact typed plan/AWBC site enums and nested coordinate tags except the
  AudioCommand/EffectPlan contradiction explicitly called out below;
- direct plan-to-AWBC equality without another digest or root map;
- one immutable admitted-generation parent and non-Serde operational wrappers;
- project-versus-producer nominal construction domains and version-1 AWBC
  `MakeRecord` domain operands; and
- no fallback, public field mutation, unchecked nominal construction, optional
  authority, compatibility alias, old reader, defaulted authority, or version
  increment.

## Residual blockers

### 1. The final lowerer cannot reach the selected constructors

`RuntimePlan`/AWBC operational DTOs are owned by `arcweft-core`, but their
actual lowerer is the separate `arcweft-runtime-plan` crate, which depends on
core. The return says the lowerer calls `pub(crate)` constructors and also
requires compile-fail proof that no public construction boundary exists. Rust
does not provide friend-crate access, so the stated P5/P6 and P8/P9 cuts cannot
compile.

### 2. Raw artifacts cannot issue their own admitted generation

The return correctly states that project and producer facts originate only in
accepted semantic HIR/registered producer facts and are assembled before raw
plan/AWBC admission. Its public `RuntimePlan::try_admit(self)` and
`AwbcProgram::try_admit(self)` instead create `AdmittedRuntimeGeneration` from
the same raw artifact's serialized declaration, with no separate accepted
fact input. This makes the quarantined artifact the source of the authority
used to accept itself.

The retained parent also owns accepted `RuntimeProjectRootFact` and
`RuntimeProducerFact` in `arcweft-runtime-plan`, while the lower
`arcweft-core` generation aggregate is specified to store those upward-owned
types. No legal dependency or exact lower-layer admitted projection is
selected.

### 3. Opaque payload recursion has no expected checked type

Current `RuntimeCheckedType::Opaque` contains only a
`RuntimeOpaqueTypeOwner`; current `RuntimeOpaqueValue` contains that owner and
an arbitrary nested `RuntimeValue`. The return says to push `OpaquePayload`
and recursively validate a producer payload contract, but defines no exact
lookup from producer/semantic identity/admission mode to one payload
`RuntimeCheckedType`. The retained producer roots describe semantic roots,
not this payload selection rule. Atomic owner-only validation and recursive
payload validation accept different values.

### 4. Two typed-site tables remain internally inconsistent

An `AwbcAudioCommand` can be referenced by multiple EffectPlan rows.
`AwbcTypedSite::AudioCommand { command, slot }` does not identify which effect
signature owns `Arg(n)`, so the promised independent resolution is not unique.
The site CSV simultaneously requires `EffectPlan::AudioValue(slot)`, while
`AWBC_SLOT_ENUMS_AND_TAGS.md` explicitly says that variant must not exist.

`RuntimeTypedExpr` requires a complete node table including root `[0]`, but
the mapping deliberately excludes legal current expression values whose
families have no `RuntimeCheckedType` case, including function, range, matrix,
and tensor values. A mandatory type row cannot be produced for those roots.

### 5. The normative test matrix does not test the normative decisions

The test matrix marks mandatory AudioCommand sites as exclusions and requests
nested-child or index-overflow failures for Option `None` and other edges that
have no child or numeric index. These rows cannot be used as acceptance
evidence until reconciled with the selected typed-site and validation rules.

## Required exact decisions

1. Select the one layer-correct construction boundary by which
   `arcweft-runtime-plan` creates core-owned `RuntimePlanTypeId`, type
   declarations, typed expressions/patterns, typed AWBC constants/patterns,
   origins, and final raw aggregates. Give exact owner modules, public versus
   private APIs, fields, accessors, checked constructors/builders, error types,
   and atomic publication behavior.
2. State why that public cross-crate boundary is raw checked data rather than
   operational authority. Replace the impossible friend-crate claim and its
   compile-fail tests with exact tests that prohibit field/default/Serde
   bypass while permitting the legitimate lowerer. Do not use caller-name,
   source-string, feature, or crate-path gates.
3. Define the final non-Serde issuer of `AdmittedRuntimeGeneration`. Identify
   the accepted-world owner of project facts, producer facts, nominal catalog,
   Character/View/custom catalog evidence, and generation declaration; give
   the exact atomic constructor/bridge and dependency direction.
4. Define the lower-layer fact projection consumed by core admission. Core
   must not depend on `arcweft-runtime-plan`, and raw serialized plan/AWBC
   declarations must not become accepted facts merely because they are
   internally consistent. Give exact projection types, constructors,
   provenance/lifetime rules, duplicate/order checks, and error mapping.
5. Replace standalone `RuntimePlan::try_admit(self)` and
   `AwbcProgram::try_admit(self)` with exact APIs that consume an already
   issued generation or perform full atomic lowering plus independent
   generation issuance before publishing an operational wrapper. If a public
   convenience consumes raw input, it must also require the independent
   admitted context; it may not derive that context from the raw input.
6. Fix plan/AWBC pair admission and runtime-driver ownership around the selected
   issuer. Specify which component owns the generation, plan, AWBC product,
   catalogs, and hot-swap inputs; how same-parent identity is preserved; and
   how restore/replay obtains the independent generation before decoding or
   executing raw artifacts.
7. Select exact opaque payload semantics. Either keep opaque payloads atomic
   after exact owner validation, or define the canonical producer-owned payload
   checked-type contract and its admission lookup. If recursive, give the
   declaration owner, root relation, generic/custom role behavior,
   ExactIdentity versus ProducerWide rule, path push, work/depth charging,
   lookup errors, and deterministic precedence. Do not infer payload type from
   producer text, nominal name, layout, or nested bytes.
8. Correct the owner order for checked validation. The final
   `RuntimeCheckedValueContext` and nominal domain issuance must land only
   after their real admitted-generation and admitted-plan/product issuers
   exist, or must use a final lower-layer context that does not name a missing
   issuer. Give exact compile-clean APIs; no placeholder context is accepted.
9. Make AudioCommand/effect resolution unique. Select the final coordinate
   owner and exact slot enums for reused commands, effect signatures, audio
   values, and indirect references. Reconcile `AWBC_SITE_RESOLUTION.csv`,
   `AWBC_SLOT_ENUMS_AND_TAGS.md`, the Rust API, verifier, lowerer, and VM; define
   bounds, aliasing, duplicate-reference, and cycle behavior.
10. Define the exact complete-node rule for `RuntimeTypedExpr` when a legal
   current `RuntimeValue` family has no closed `RuntimeCheckedType`. Select
   rejection, an exact checked-type algebra extension, or a final explicit
   non-root/non-node grammar. Give the complete exhaustive table and ensure a
   root `[0]` rule can be implemented for every admitted expression.
11. Replace every contradictory normative test row. The final matrix must
    align with mandatory AudioCommand/effect sites, payload-free Option
    branches, nonnumeric edges, opaque atomic/recursive semantics, and the
    legitimate external lowerer construction boundary.
12. Give a compile-clean implementation/deletion order starting with the
    cross-crate construction API and independent generation issuer. No
    placeholder generation, self-admission fallback, temporary public fields,
    compatibility wrapper, dual reader, or disabled validation success branch
    is accepted.
13. Update the complete producer/consumer/deletion inventory for core,
    runtime-plan, runtime-driver, compiler, verifier, VM, AOT, snapshot,
    restore/replay, dialogue, View, tests, and maintained documentation.

## Required tests

The returned design must require executable evidence that:

- the real `arcweft-runtime-plan` lowerer constructs every core-owned typed DTO
  through the selected checked cross-crate boundary;
- unrelated callers cannot bypass validation through public fields, Default,
  unchecked constructors, derived raw DTOs, or alternate Serde shapes;
- a raw plan/AWBC artifact cannot issue or alter the accepted project/producer
  facts against which it is verified, even when all of its self-declared rows
  are changed consistently;
- accepted-world facts project once into the core-owned admitted generation,
  and cross-generation plan/AWBC/catalog/restore inputs fail before execution;
- opaque payload validation follows the one selected atomic or recursive rule
  with exact owner, path, depth/work, and error precedence;
- every reused AudioCommand resolves through one unambiguous effect/signature
  coordinate and malformed aliases/references fail deterministically;
- every legal admitted RuntimeExpr has the exact selected root/node fact set;
  and nonrepresentable families follow the selected explicit rule; and
- each numbered implementation phase compiles and the raw execution APIs are
  deleted in the same phase their admitted replacement becomes available.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.2.1.1.1.1-external-lowering-and-independent-generation-admission-authority-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, the exact current request copy/hash,
current-main Git/source evidence, exact Rust-shaped APIs, corrected typed-site
tables, a consistent test matrix, complete inventory, and compile-clean order.
Keep every sidecar inside the ZIP.
