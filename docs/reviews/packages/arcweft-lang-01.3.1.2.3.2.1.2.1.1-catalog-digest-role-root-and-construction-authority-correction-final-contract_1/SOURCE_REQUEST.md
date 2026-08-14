# Lang-01.3.1.2.3.2.1.2.1.1 — catalog-digest, role-root, and construction-authority correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.2.1.1. It is a narrow mandatory correction to
the returned Lang-01.3.1.2.3.2.1.2.1 generation-bound producer-root and AWBC
admission-authority contract.

The retained returned archive is
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1-generation-bound-producer-root-and-awbc-admission-authority-correction-final-contract.zip`,
SHA-256
`aa43429b6ffe5aac6489c94c7ff7a117ca1bbd43c764fed6ff4a1f3b5d540e06`.
Its searchable frozen mirror is the sibling package directory.

The implementation audit used Git commit
`175a74da637ca5f455abdefda49c6b62897b00e2` on `main`, equal to
`origin/main`, with an initially clean production tree. The returned archive
was then retained and extracted as uncommitted intake evidence.

This is a design-only request. It must not return production code, a patch, an
overlay, or a compatibility path. Every Arcweft-owned schema, ABI, codec,
digest-domain, protocol, and persistence version remains exactly `1`.

## Accepted substrate that remains fixed

Do not redesign these returned decisions without a concrete current-source
defect:

- one serialized `RuntimeGenerationContractDeclaration` shared by raw
  `RuntimePlan` and `AwbcProgram`;
- non-circular producer payload roots and exact derived-versus-claimed nominal
  authorization equality;
- one non-Serde `AdmittedRuntimeGeneration` aggregate reused by admitted plan
  and plan-paired AWBC products;
- standalone AWBC admission from its complete embedded generation contract;
- raw plans/programs remain untrusted quarantine and operational publication
  requires consuming full admission;
- `RuntimeGenerationIdentity` is the BLAKE3 digest of the canonical contract
  body under the fixed version-`1` domain;
- CharacterDialogue uses the exact `std.character_dialogue` producer, derived
  Style, computed custom-field digest, nested Option/voice representation, and
  generation-bound Character/View catalog wrappers;
- producer-ID lookup returns a non-exclusive admitted-shape view rather than a
  caller-exclusive credential;
- `RuntimeNominalRecordValue::try_from_accepted_layout` remains crate-private;
- all A1-A3 record carrier/layout/field-ID results and the accepted exact
  Variant owner/ordinal/name/payload behavior; and
- no `Dynamic`, source-name reconstruction, copied operational side table,
  optional authority, fallback, dual reader, compatibility alias, or version
  increment.

The lower shared vocabulary is already mechanically determined and may be
implemented while this request is outstanding: `CharacterDialogueCustomFieldId`
and `CharacterDialogueRuntimeRole` are owned by
`arcweft-interaction-model`; the former preserves the accepted
`character_dialogue_field.*` family, `PublicId` validation, and 128-byte limit.

## Split reason

The returned package reports `READY_FOR_IMPLEMENTATION`, but five
result-changing boundaries remain unspecified.

First, `RuntimeCharacterCatalogDigest` and `RuntimeViewCatalogDigest` are
stored in the generation contract and later recomputed by `CharacterCatalog`
and `ViewRegistry`, but neither current owner has such a canonical digest and
the return defines no domain-separated transcript. Selecting which manifest or
View fields participate, identity encodings and order, treatment of anonymous
Views, retired/tombstone rows, implementation IDs, or limits changes generation
identity and catalog admission.

Second, the six authored CharacterDialogue base roles remain abstract
"accepted typed declarations". The return does not define the Rust-shaped
`CharacterDialogueRuntimeRoleDeclaration`, its standard registration source,
or the exact closed semantic and runtime type for Stage, Portrait, Focus,
Cleanup, Hook, and RichText. Current production still has only `TypeKind::Named`
placeholders, so materially different role meanings satisfy the prose.

Third, plan/AWBC root correlation is not mechanically closed.
`RuntimeProjectRootFact` and `RuntimeProducerFact` are referenced but undefined;
there is no exact root-ID derivation grammar, per-table retained coordinate, or
exhaustive mapping from plan and AWBC tables to generation roots. Different
inventories change generation bytes and reachable/unreachable catalog results.

Fourth, project nominal construction has no issued authority. The return names
`RuntimeNominalRecordAdmissionDomain` but does not define it, exposes only
producer lookup on `AdmittedRuntimeGeneration`, and leaves AWBC `MakeRecord`
without a project-versus-producer domain coordinate. An implementation cannot
decide which admitted shape authorizes construction without changing results.

Fifth, unique-Choice validation is stated semantically but has no exact typed
owner/API. Current core exposes boolean `RuntimeCheckedType::accepts_value`.
The return references `RuntimeCheckedTypeError::{ChoiceNoMatch,
ChoiceAmbiguous}` without shaping that error, branch mismatch evidence, shared
work-budget behavior, or its mapping into `RuntimeNominalRecordTreeError`.
Those errors and branch outcomes are observable at dialogue, restore, and View
boundaries.

These are not private implementation choices. They affect canonical bytes,
generation identities, admission success, nominal construction authority,
and typed error precedence.

## Required exact decisions

1. Define the canonical `CharacterCatalog` digest owner and transcript. Pin
   the version-`1` BLAKE3 domain, exact included fields, scalar/string/optional/
   sequence encoding, sort order, duplicate handling, limits, treatment of
   aliases/looks/defaults/retired rows, and typed errors. State which existing
   manifest/catalog value is the source and prohibit display/debug/Serde bytes
   as implicit authority.
2. Define the canonical `ViewRegistry` digest owner and transcript with the
   same precision. Close authored versus anonymous/generated View identities,
   accepted `RuntimeViewId` projection, descriptor/implementation fields,
   insertion order, tombstones/retirement, duplicate identities, limits, and
   typed errors.
3. Give exact Rust-shaped APIs by which `CharacterCatalog` and `ViewRegistry`
   recompute their typed digests and issue generation-bound admitted wrappers.
   Pin digest mismatch and generation mismatch precedence. No caller-supplied
   digest constructor is accepted.
4. Define `CharacterDialogueRuntimeRoleDeclaration` (or the uniquely selected
   equivalent) with exact owner, private fields, derives, constructor,
   accessors, source evidence, error enum, and registration/publication path.
5. For Stage, Portrait, Focus, Cleanup, Hook, and RichText, provide an exact
   table of accepted semantic source, closed `TypeKind`, generic arguments,
   nominal/opaque identity and producer where applicable, final
   `RuntimeCheckedType`, and source/world evidence. Define Style only as the
   ordered `Choice([EntityRef, RichText])` projection.
6. Define how the standard library registers exactly one declaration for each
   base role and how aliases/normalization substitute the role coordinate.
   `Named`, string tables, display labels, or path recognition must not remain
   a success path.
7. Define `RuntimeProjectRootFact`, `RuntimeProducerFact`, and every root
   coordinate type used by the runtime-plan bridge. Pin exact owner, fields,
   derives, constructor, and source evidence. Define a project-root-capable
   typed error owner: the returned `RuntimeProducerRootError` carries only
   `RuntimeProducerRootId` and cannot represent the separately required
   `RuntimeProjectRootId` order, duplicate, unresolved, or lookup failures.
8. Define root-ID creation. If it is a digest, pin its domain and canonical
   typed coordinate grammar. If it is an accepted semantic ID, identify the
   exact existing owner and lossless projection. Core must not derive IDs from
   display names, dense indices, iteration order, or debug output.
9. Supply an exhaustive table mapping every current `RuntimePlan` typed
   publication boundary to one project or producer root coordinate, including
   entries, callable/flow signatures, frames/locals, constants, patterns,
   roots, resources, reducers, View inputs, streams/tasks, replay/save-visible
   slots, and any deliberate exclusions.
10. Supply the equivalent exhaustive AWBC table mapping and exact plan-to-AWBC
    coordinate preservation rule. State how dense IDs resolve to retained
    semantic coordinates and how exact equality is checked.
11. Define `RuntimeNominalRecordAdmissionDomain<'generation>` and the project
    construction authority API. Pin whether project shapes are issued by
    `AdmittedRuntimeGeneration`, `AdmittedRuntimePlan`, or an execution-site
    coordinate view, with exact lifetimes, fields, constructors, accessors,
    errors, and no generation-erasing escape.
12. Define the AWBC `MakeRecord` authority coordinate and wire/lowering rule.
    State exactly how an instruction selects project versus one producer
    domain, how verifier/admission validates it, and how VM obtains the
    corresponding admitted shape before calling the crate-private value
    constructor.
13. Define the typed checked-value validation owner/API that replaces boolean
    acceptance at authority-bearing boundaries. Shape
    `RuntimeCheckedTypeError`, `ChoiceNoMatch`, `ChoiceAmbiguous`, branch
    mismatch summaries, `RuntimeCheckedTypePath`, `RuntimeValuePath`, depth and
    shared work-budget evidence, and deterministic first-error order.
14. Define exact mapping from checked-value failures into nominal-tree,
    dialogue, restore, replay, View, plan/AWBC admission, and VM errors without
    string flattening. State where boolean convenience may remain, if anywhere.
15. Correct the compile-clean implementation order so the lower shared
    `CharacterDialogueCustomFieldId` and role enum land before core declarations,
    then canonical scalar/checked-type substrate, raw declarations, semantic
    role/catalog projection, root correlation, admission, AWBC, execution cut,
    dialogue, and final unchecked-constructor deletion.

## Required precedence

### Catalog admission

1. catalog-local syntax and structural validation;
2. canonical identity/order/duplicate checks;
3. canonical transcript construction under fixed domain `1`;
4. digest recomputation;
5. declared-versus-actual digest comparison;
6. generation identity comparison;
7. referenced View/character relationship checks;
8. atomic admitted-wrapper publication.

### Checked-value and Choice validation

1. nesting and shared work budget;
2. outer runtime shape;
3. checked owner;
4. ordinal and name;
5. payload presence;
6. recursive payload;
7. evaluate every Choice alternative in source order under the shared budget;
8. zero-match ordered branch evidence or first two matching indices;
9. nominal admitted-shape lookup/tree validation;
10. domain publication.

### `MakeRecord`

1. AWBC structural instruction/type/reference checks;
2. admitted generation and execution-site root coordinate;
3. project/producer domain selection;
4. exact nominal/semantic/layout lookup;
5. domain authorization membership;
6. field count and field checked-value validation;
7. crate-private construction;
8. register publication.

No later error is observable after an earlier failure.

## Required producer and consumer inventory

Inspect and close at least:

- `crates/arcweft-character` manifest/catalog identities and every field that
  may affect runtime dialogue admission;
- `crates/arcweft-view` registry, descriptors, generated/anonymous identities,
  implementation/product metadata, retirement, and current consumers;
- `crates/arcweft-interaction-model`, `arcweft-dialogue`, and
  `arcweft-lang-sema` CharacterDialogue ID/role declarations and registrations;
- current standard callable family rows and every `TypeKind::Named` dialogue
  role placeholder;
- `arcweft-runtime-plan::{semantic_facts,lower,awbc_lower,inventory}` and every
  typed plan/AWBC table that can construct, persist, restore, pattern-match, or
  publish a RuntimeValue;
- `arcweft-core::{pattern,plan,value::nominal_record}` and
  `arcweft-core::awbc::{schema,codec,verify,type_projection,vm,fiber,
  product_step}`;
- AWBC `MakeRecord` schema, lowering, verifier, VM, AOT/JIT/codegen consumers;
- dialogue schema/typed-value/patch, runtime-driver restore/replay/View, bundle,
  save, native/Web/headless player, agent/CLI, and test fixture boundaries; and
- all raw scalar/digest constructors, boolean `accepts_value` uses, root-ID
  derivations, and project nominal construction call sites.

## Required tests

- changing each included CharacterCatalog field changes its digest, while
  source-only/non-runtime evidence does not;
- every character ordering, duplicate, limit, malformed identity, and retired
  row case follows the fixed transcript and error order;
- the same matrix for ViewRegistry, including anonymous/generated Views and
  implementation metadata;
- cross-generation or stale Character/View digests reject before schema/value
  publication;
- all six base roles register once from typed facts and project to the exact
  required closed semantic/runtime types without name recognition;
- missing, duplicate, wrong-world, unresolved, leaked role-coordinate, and
  wrong closed-type role declarations retain typed source evidence;
- Style is exactly ordered `Choice([EntityRef, RichText])` and cannot be
  authored independently;
- every plan root and AWBC root round-trips the same typed coordinate and root
  ID; table omission, duplication, substitution, or dense-index reassignment
  fails correlation deterministically;
- a nominal reachable only through one plan/AWBC table is admitted, while
  removing that typed root makes the row unreachable;
- project `MakeRecord` succeeds only through project authority, producer
  construction only through the exact producer, and cross-domain substitution
  fails before construction;
- malformed/forged `MakeRecord` domain coordinates fail verifier/admission;
- Choice zero-match retains ordered typed branch failures; ambiguous Choice
  reports the first two matching indices; a later malformed/deep branch cannot
  be hidden by an earlier success;
- one shared work budget and nesting limit apply across Choice branches and
  nominal traversal;
- nominal-tree, dialogue, restore, View, and VM errors preserve typed checked
  sources and paths;
- lower shared ID/role relocation preserves accepted serde spelling, 128-byte
  limit, family validation, and public re-exports; and
- focused tests, workspace check, Clippy, structure audit, codec/golden/tamper
  tests, and applicable Tier 2 commands are enumerated with no version change.

## Constraints and non-goals

- Do not reopen the accepted generation body, producer-root closure, admitted
  aggregate, AWBC embedding, nested voice, or opaque tuple decisions.
- Do not invent a second semantic generation ID, catalog, root map, role table,
  custom digest, or operational handle.
- Do not use Serde/debug/display/source text or iteration order as canonical
  digest or root identity unless an exact retained codec is explicitly selected
  and justified.
- Do not treat `RuntimeNominalRecordLayout`, a catalog key, a root ID, a
  producer ID, or a digest as an operational capability.
- Do not make dialogue depend on runtime-plan, sema, compiler, HIR, syntax,
  runtime-driver, View, or another higher layer.
- Do not retain `Named` role success, boolean-only authority validation,
  generation-blind nominal construction, raw AWBC execution, or a fallback
  project/producer domain.
- Do not add compatibility aliases, old readers, optional fields, defaults,
  migrations, or any Arcweft-owned version other than `1`.
- Do not return production code or an implementation overlay.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owners/APIs/errors,
canonical Character/View digest and root-coordinate grammars, exact role and
`MakeRecord` tables, checked-value/Choice validation semantics, complete
inventories and test matrices, deterministic precedence, and a compile-clean
implementation/deletion order. Keep every sidecar inside the ZIP.
