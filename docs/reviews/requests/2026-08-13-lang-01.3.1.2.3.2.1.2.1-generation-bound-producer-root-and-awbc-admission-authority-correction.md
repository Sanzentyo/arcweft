# Lang-01.3.1.2.3.2.1.2.1 — generation-bound producer-root and AWBC admission-authority correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.2.1. It is a narrow mandatory correction to the
returned Lang-01.3.1.2.3.2.1.2 external nominal-value admission contract. It
must return before that package's G2 catalog authority or parent A4 unchecked
constructor deletion can be accepted.

The retained returned authority is
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2-nominal-runtime-value-external-admission-and-dialogue-layout-authority-correction-final-contract.zip`,
SHA-256
`7a7001cba41f312d428a88589877ce48eb3bb6734aff234b72601d7bfa6a9d70`.
Its searchable frozen mirror is the sibling package directory.

The implementation audit used Git commit
`50771a19f57f86570837f616a66252be24e77e0c` on `main`, equal to
`origin/main`, plus the independently accepted G1 closed-variant correction at
`1648894fbfc38ba623d1b01c6001fbd55b67b10b`. The returned package was pinned
to its production parent `98ccafa5f0113a50f8a0f5e985df5f695c401588`;
the intervening commits implement A3, record its blocker, and implement only
that G1 correction, without closing the gaps below.

This is a design-only request. It must not return production code, a patch,
an overlay, or a compatibility path. Every Arcweft-owned schema, ABI, codec,
digest-domain, protocol, and persistence version remains exactly `1`.

## Accepted substrate that remains fixed

The return must close the missing authority evidence without reopening these
accepted decisions unless a concrete current-source defect is demonstrated:

- parent A1-A3 nominal layout, expression, field-ID, anonymous-record, and
  record-column contracts;
- crate-private `RuntimeNominalRecordValue::try_from_accepted_layout` and
  deletion of public unchecked `new`/`validate_shape` at the final A4 cut;
- a non-Serde operational nominal admission handle, rather than a public raw
  nominal/layout/fields constructor;
- `RuntimeNominalRecordLayout::try_from_checked_projection` as a forgeable
  descriptor-construction API, never itself an operational capability;
- CharacterDialogue's exact opaque owner with producer-owned tuple payload,
  tuple2 custom entries, direct inline-failure variant, descriptor-aware
  transforms, and atomic patch publication;
- no `RuntimeCheckedType::Dynamic`, producerless opaque fallback, source/name/
  hash reconstruction, copied descriptor table, or dual reader;
- `arcweft-dialogue` must not depend on `arcweft-runtime-plan`, compiler,
  sema, HIR, syntax, runtime driver, or another higher layer; and
- the independently implemented G1 correction that makes nominal variants
  check exact owner, ordinal, name, payload presence, and payload type.

## Split reason

The returned `.1.2` package defines serialized
`RuntimeNominalRecordProducerDeclaration` rows as only a producer ID plus
catalog keys. It says `RuntimePlan::try_admit` verifies that every authorization
row was emitted from accepted closed producer payload facts, but the proposed
`RuntimePlan` API stores no producer payload contract, role/custom root set, or
other independent evidence from which that claim can be recomputed. A raw
Serde plan can therefore add a producer/key pair that resolves to a global
layout. If admitted, the resulting producer handle can construct a nominal
value under a domain that was never authorized. Reachability from a producer
row is circular evidence, not admission.

The proposed public `CharacterDialogueRuntimeRoleTypes::new` likewise accepts
arbitrary `RuntimeCheckedType` values. The schema only preflights nominal
nodes, so a caller can designate unrelated or primitive types as style,
rich-text, stage, or other roles and still construct an exact
`std.character_dialogue` opaque value. Neither the role type set nor the
custom-field catalog is correlated to an admitted plan generation.

Current semantic facts do not supply the canonical root types assumed by the
package. The standard dialogue/View roles are currently expressed through
`TypeKind::Named` values such as `DialogueStage` and `DialogueContent`, while
the current runtime projection rejects unresolved `Named` shapes with missing
opaque-producer evidence. Thus “derive transitive producer keys from accepted
closed role/custom facts” is not mechanically implementable until the unique
role/custom checked-type publication owner and projection are fixed.

The current custom catalog accepts a caller-supplied digest independently of
its descriptors. The returned API retains that digest while changing each
descriptor's semantic content to a closed checked type, but gives no canonical
digest grammar or proof that the digest and descriptor map describe one
generation.

Finally, product execution does not necessarily carry a `RuntimePlan`.
`AwbcProgram` is independently serialized, verified, and executed through VM,
fiber, and `AwbcProductStepExecutor` APIs. The `.1.2` package requires admitted
catalog handles during AWBC nominal construction and CharacterDialogue
activation but does not define where the independently executable AWBC
artifact carries producer-root evidence, how it gains operational admission,
or how direct VM/product-step APIs are prevented from bypassing
`RuntimePlan::try_admit`.

These choices affect serialized plan/AWBC authority, exact admission results,
generation isolation, CharacterDialogue type meaning, artifact verification,
restore/activation, and public execution APIs. They cannot be safely inferred
as private implementation details.

## Required exact decisions

1. Define the single non-circular producer payload-contract declaration that
   authorizes nominal catalog keys. Give exact owner module, Rust type, private
   fields, derives, Serde shape, constructors, accessors, and validation
   errors. Producer rows must be validated from independent canonical roots;
   they must not authorize themselves.
2. Define the exact root vocabulary for each producer. Close whether a root is
   a closed `RuntimeCheckedType`, a typed producer payload contract, a role/
   custom coordinate mapped to one checked type, or another lower-layer type.
   Define transitive nominal traversal, Choice/opaque/variant handling, depth
   and work limits, ordering, duplicate handling, and first-error precedence.
3. Define how the compiler/runtime-plan bridge obtains canonical producer
   roots from accepted semantic facts. Identify the existing or new accepted
   fact owner, exact projection API, source evidence, and error mapping. Do not
   recognize role names or reconstruct types from strings.
4. Close every standard CharacterDialogue role type: stage, portrait, focus,
   cleanup, hook, style, and rich text. State the exact semantic owner and
   final closed `RuntimeCheckedType` for each, including how current
   `TypeKind::Named` rows become executable evidence or are directly replaced.
5. Define the final `CharacterDialogueRuntimeRoleTypes` construction boundary.
   A public arbitrary seven-type constructor is not an admitted authority.
   Specify whether the object is issued by plan/AWBC admission, borrowed from
   an admitted producer contract, or built only by an upper bridge and then
   revalidated against immutable canonical evidence.
6. Define the final CharacterDialogue custom-field catalog declaration,
   canonical digest grammar, ordering, limits, and validation. The digest must
   be derived from or verified against field ID, exact checked type,
   clearability, accepted Views, and every other semantic field; it must not be
   an unrelated caller assertion.
7. Fix the exact CharacterDialogue voice runtime representation. Close whether
   tuple index 5 is nested `Option::{None, Some(CharacterDialogueVoice::{Auto,
   Id})}` or one flat three-case variant, including exact owners, ordinals,
   names, payloads, and canonical bytes. Define exact branch selection and
   first-error rules for every Variant and Choice position; do not leave
   “shallow compatible” as an unowned predicate.
8. Define the generation identity/correlation model shared by the admitted
   plan or product, nominal catalog, producer capability, role types, custom
   catalog, Character/View catalogs, restore context, and runtime activation.
   Lifetimes alone are not generation identity. Give exact scalar owner,
   construction/allocation source, equality rules, traits, and mismatch
   errors, or prove a single owning aggregate makes a separate scalar
   unnecessary.
9. Define the exact `RuntimePlan` serialized fields and `try_admit` algorithm
   that independently validate layouts, project roots, producer roots,
   authorization closure, reachability, generation correlation, and atomic
   publication. Pin error precedence and prohibit producer-row-only
   reachability from legitimizing a row.
10. Define the AWBC product equivalent. State exactly which catalog layouts,
   producer root contracts, role/custom facts, and generation evidence are
   serialized in `AwbcProgram` or a single paired product artifact, and how
   canonical codec bytes/digest include them while all version numbers stay
   `1`.
11. Define the non-Serde admitted AWBC/product wrapper and exact admission API,
    including verifier order, catalog construction, producer handles, and
    relationship to `AdmittedRuntimePlan`. Do not create two independently
    disagreeing operational catalogs for one generation.
12. Inventory and close every direct execution API that currently accepts raw
    `AwbcProgram`, including VM step, fiber construction/resume,
    `AwbcProductStepExecutor`, runtime-driver session construction/hot swap,
    player/native/Web/headless entry points, tests, and restore. State which
    APIs become crate-private, accept an admitted wrapper, or borrow from one
    single owning generation image.
13. Define CharacterDialogue schema construction only from the admitted
    generation aggregate. Pin how exact producer identity, role types, custom
    catalog, Character catalog, View catalog, and generation evidence are
    compared before any encode/decode/digest/patch operation.
14. Define typed error mapping and source/path evidence for malformed producer
    roots, missing canonical role facts, digest mismatch, cross-generation
    catalog use, raw-plan admission, AWBC product admission, restore, replay,
    View activation, and CharacterDialogue construction.
15. Decide whether `.1.2`'s public ID-only `RuntimeNominalRecordCatalog::producer`
   returns a non-exclusive admitted-shape handle or whether producer-owned
   authority requires a sealed credential or generation-specific issuance. If
   it is producer-specific authority, forbid ID-only lookup. In either result,
   the handle must have no public fields, constructor, Serde, or Default; raw
   descriptor/key/layout data cannot create it, and generation escape or reuse
   must be impossible by construction or exact validation.
16. Give the exact deletion set and compile-clean implementation order for
    self-authorizing producer rows, public arbitrary role-type construction,
    caller-supplied unverified custom digest, raw executable plan/AWBC APIs,
    duplicate operational catalogs, and any generation-blind success path.

## Required precedence

The return must close at least these orders.

### Plan/product admission

1. syntax/codec/header and fixed version-`1` checks;
2. existing structural/bytecode/plan verification;
3. canonical producer-root and role/custom declaration validation;
4. canonical custom digest verification;
5. catalog key/descriptor scalar and structural consistency;
6. independent project and producer root traversal;
7. exact authorization-set equality or the stricter selected relation;
8. missing, extra, conflicting, and unreachable rows;
9. generation correlation; and
10. atomic issuance of one admitted execution authority.

### Producer lookup and CharacterDialogue schema construction

1. admitted generation identity;
2. exact producer identity;
3. canonical role/custom root identity and digest;
4. nominal/semantic/layout lookup and producer authorization;
5. nested field validation; and
6. schema publication or typed failure.

No error after operational publication may reveal that an earlier authority
check was skipped.

## Required producer and consumer inventory

Inspect and close at least:

- `arcweft-core::{plan, plan::entry_inventory, value::nominal_record}`;
- `arcweft-core::awbc::{schema, codec, verify, type_projection, vm, fiber,
  product_step}`;
- runtime-plan semantic facts, final lowering, AWBC lowering/inventory, and
  compiler project/runtime bridges;
- sema accepted nominal/opaque facts and every standard dialogue role source;
- `arcweft-dialogue::character_dialogue::{schema, typed_value, patch}` plus
  `CharacterDialogue::{try_new,digest,canonical_bytes,patched}`;
- runtime-driver generation image, session construction, hot swap, restore,
  root/replay, View runtime, and persistence;
- bundle/AWFB construction and decode, save/session snapshots, native/Web/
  headless players, agent runner/MCP/CLI, runtime accelerator, JIT/codegen, and
  test helpers; and
- `ArcweftRuntimeExecutor::{from_awbc_product,
  from_awbc_product_function, replace_product_awbc_program}`,
  `Engine::{new, for_flow, for_entry}`, `BytecodeProgram::{from_runtime_plan,
  into_runtime_plan}`, and every AOT or plan/AWBC conversion boundary; and
- all direct raw `RuntimePlan` and `AwbcProgram` execution call sites.

## Required tests

- a raw plan cannot authorize an extra producer/key pair absent from canonical
  producer roots;
- missing and extra producer authorization keys fail deterministically;
- producer rows do not make otherwise unreachable rows reachable;
- arbitrary role types (for example `Bool` as style) cannot construct a
  CharacterDialogue schema or value;
- all seven standard roles project from one accepted fact owner without name
  recognition and yield exact closed types;
- changing only a custom descriptor field changes its canonical digest;
- a caller-supplied mismatched custom digest is impossible or rejected;
- plan, AWBC, producer capability, role/custom catalogs, Character/View
  catalogs, restore state, and activation from different generations reject;
- raw `RuntimePlan` and raw `AwbcProgram` cannot reach execution/publication;
- direct VM/fiber/product-step construction requires the selected admitted
  product authority;
- a canonical plan-to-AWBC product has one equivalent producer authorization
  closure and cannot create two disagreeing operational catalogs;
- malformed AWBC producer roots/catalog data fail before VM/fiber construction;
- CharacterDialogue encode/decode/digest/equality/hash/patch and restore use
  the same admitted generation authority;
- external handle fields/constructors remain compile-fail, while obsolete
  internal raw execution call sites are closed by workspace compilation and
  typed tests rather than source-spelling gates;
- all A1-A3 and `.1.2` representation/transform tests remain green; and
- focused core/runtime-plan/compiler/sema/dialogue/AWBC/driver/bundle/save/
  player tests, workspace check, Clippy, structural audit, and applicable Tier
  2 gates are specified.

## Constraints and non-goals

- Do not make `RuntimeNominalRecordValue::try_from_accepted_layout` public.
- Do not treat a public `RuntimeNominalRecordLayout`, producer row, role name,
  checked-type string, layout hash, semantic digest, or custom digest as an
  operational capability.
- Do not let producer authorization evidence authorize itself.
- Do not claim that callers cannot construct raw serialized plans or programs.
  Treat raw `RuntimePlan` and `AwbcProgram` as untrusted quarantined data until
  exact whole-program reconciliation issues an operational wrapper. A public
  convenience API may consume raw input and perform full atomic admission
  internally before publishing an executor; it may not borrow/store raw input,
  substitute `verify()` for admission, or expose a raw VM/fiber constructor,
  `Deref`, `into_inner`, replacement, or restore bypass.
- Do not add a global mutable registry, lookup by producer string, copied
  descriptor/role table, optional generation token, or runtime fallback.
- Do not make `arcweft-dialogue` depend on runtime-plan/compiler/sema/HIR or
  runtime-driver; place shared vocabulary in a permitted lower owner and have
  upper layers populate it.
- Do not leave raw `AwbcProgram` executable while only `RuntimePlan` is
  admitted.
- Do not add `RuntimeCheckedType::Dynamic`, a producerless opaque value, an old
  CharacterDialogue nominal reader, or a dialogue-only unchecked constructor.
- Do not reopen accepted A1-A3 carrier/layout/field-ID rules or the `.1.2`
  opaque tuple/descriptor-aware transformation result without a concrete
  repository-evidenced defect.
- Do not allocate or increment any Arcweft-owned version; every version stays
  exactly `1` and unreleased wire/storage models are replaced directly.
- Do not include production code or a patch in the returned archive.
- Place all authority-bearing plan/AWBC fields, producer roots, role/custom
  correlation, mandatory decode/restore admission, and execution-API migration
  in A4. A6 is only the exhaustive codec/golden/tamper/canonical-byte audit; it
  must not introduce authority or preserve a raw execution path until then.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.2.1-generation-bound-producer-root-and-awbc-admission-authority-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owner/API/Serde/error and
generation-correlation decisions, plan and AWBC canonical grammars, complete
producer/consumer/deletion inventories, deterministic precedence, a
compile-clean implementation order, and positive/negative test matrices. Keep
every sidecar inside the ZIP.
