# Normative delta against returned Lang-01.5.1.2

## 1. Baseline

Baseline archive:

```text
docs/reviews/arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip
SHA-256: CA72FD70C657A11B7BECDB331D131177B6DEFD6094D034BBECFC3AF1A232E1C0
repository blob: 4b9a0303f33744f0504b2ed79a2f61397f0f71ff
```

The baseline is authoritative except for the rows amended below. “Unchanged”
means the prior row remains normative without reinterpretation.

## 2. `CONTENT_ROOT_FAMILIES.md` — resolution precedence

| Baseline row | Baseline meaning | Delta |
|---|---|---|
| P1 | parse first family segment with typed reference owner | **Unchanged.** The final owner has no Source category. |
| P2 | Character is reserved file-backed and does not fall through | **Unchanged.** |
| P3 | source-owned family is reserved and resolves to source symbol | **Amended.** Rename class to authored-entity; inventory is exactly Flow/View/Action/Activity/Asset/Signal/Metric/Layer. Source is absent. |
| P4 | other families consult exact configured-resource index | **Unchanged**, but it cannot be used as a convention-based Source replacement. |
| P5 | known invalid built-in without configured declaration is wrong family | **Unchanged.** |
| P6 | no built-in/configured category is unknown | **Unchanged.** Former Source references use this ordinary path when no final target exists. |
| P7 | duplicate configured declarations are integrity failure | **Unchanged.** |
| Alias/visibility paragraph | canonical target plus occurrence spans; no prefix visibility shortcut | **Unchanged**, with no Source binding/alias category. |

## 3. `CONTENT_ROOT_FAMILIES.md` — every family-table row

| Baseline family row | Status | Final row |
|---|---|---|
| `character` | unchanged | file-backed exact `CharacterId`/validated package |
| `flow` | terminology-only amendment | authored entity `Flow`, exact `EntityId` |
| `view` | terminology-only amendment | authored entity `View`, exact `EntityId` |
| `action` | terminology-only amendment | authored entity `Action`, exact `EntityId` |
| `activity` | terminology-only amendment | authored entity `Activity`, exact `EntityId` |
| `source` | **deleted** | no family, target, alias, tag, reservation, or positive test |
| `asset` | terminology-only amendment | authored entity `Asset`, exact `EntityId` |
| `signal` | terminology-only amendment | authored entity `Signal`, exact `EntityId` |
| `metric` | terminology-only amendment | authored entity `Metric`, exact `EntityId` |
| `layer` | terminology-only amendment | authored entity `Layer`, exact `EntityId` |
| accepted `res` family | unchanged | exact `ResourceDeclarationIdentity` |
| `entry` | unchanged | invalid root; profile selects entry separately |
| `content` | **strengthened** | source declaration deleted; direct manifest fact, no compatibility entity |
| `choice`, `choice_option` | unchanged | invalid nested flow products |
| `dialogue_line`, `text` | unchanged | invalid generated/scoped products |
| `input`, `button`, `style` | unchanged | invalid scoped/runtime products |
| `scene`, `capture`, `hook` | unchanged | invalid runtime/tooling products |
| `slot`, `target` | unchanged | invalid scoped presentation identities |
| presentation target | unchanged | retained dependency, not root |
| scroll region | unchanged | View-scoped identity, not root |
| old `image`/audio/motion/rig source families | unchanged | exact configured resource only |
| proof/type/function names | **clarified** | all ordinary/external Stream callables are wrong symbol category regardless of return/execution mode |
| unknown prefix | **clarified** | no guessing; a removed Source spelling receives ordinary unknown/wrong-target resolution |
| owner behavior: inherent `EntityKind` method | **amended** | exhaustive final mapping contains no Source arm |
| owner behavior: resource index lookup | unchanged | exact declaration lookup |
| owner behavior: no duplicate loader/compiler/LSP table | unchanged | additionally prohibits callable/name/return-type helper |

## 4. `RUST_SHAPES.md` delta

| Baseline shape/row | Delta |
|---|---|
| `ProjectTopologyRevision` | unchanged |
| `ProjectBinaryResource` | unchanged |
| `ContentRootOccurrenceId` | unchanged |
| source-named authored-family enum | **renamed without alias** to `AuthoredContentRootFamily` |
| enum variants Flow/View/Action/Activity/Asset/Signal/Metric/Layer | unchanged |
| enum Source variant | **deleted** |
| `AcceptedContentRootTarget::Character` | unchanged |
| source-named entity target variant | **renamed without alias** to `AuthoredEntity` |
| Source-family target possibility | **deleted** |
| `ConfiguredResource` target | unchanged |
| explicit closed `ContentRootFamily` | **added** to make final classification/identity exact |
| `AcceptedContentRootTarget::family()` | **added** as owner-local projection |
| `ContentRootReferenceKind` | unchanged |
| `AbsentCharacterRoot` | unchanged |
| candidate/accepted state enums | unchanged; “source-owned” prose becomes “authored entity” |
| profile selection/source provenance | unchanged |
| candidate/accepted unit/root inventory | unchanged except final target/family type names |
| `AcceptedCharacterPackage` | unchanged |
| `CharacterPackage` validation | unchanged |
| loader text/binary overlay and exact accessors | unchanged |
| launch source-map types/accessors | unchanged |
| `EntityKind::content_root_class` | amended final mapping; no Source |
| `ContentRootReferenceFact/Inventory` | unchanged structure; target inventory cannot contain Source/callable |
| optional-absence reservation | unchanged; Character only |
| callable classification | **new explicit negative:** no callable target variant and no Stream-derived family |
| `ProjectSemanticIndexInput.content` | retained |
| ProjectIndex source-content relation | **replaced** by direct manifest-owned unit/root facts and typed graph endpoints; no `EntityKind::Content` node |
| dependency direction/Sans-I/O | unchanged |

The old source-named type/variant spellings are not compatibility aliases. Code
using them must fail to compile after the migration.

## 5. `REVISION_AND_ADMISSION.md` delta

| Baseline row/section | Delta |
|---|---|
| header/transcript version | unchanged |
| present tags `0x01`–`0x05` | unchanged |
| semantic tag `0x20` | unchanged |
| absence tag `0x80` | unchanged |
| canonical order/duplicate policy | unchanged |
| included/excluded inputs | unchanged; explicitly excludes Source family/tag and callable execution mode |
| Character path/acquisition | unchanged |
| required/optional Character table | unchanged |
| “Source-owned/configured-resource roots are SemanticPending” | **amended** to authored-entity/configured-resource roots |
| semantic stage Source-owned resolution | **amended** to the eight final authored families; no Source reservation/lookup |
| typed reference inventory | unchanged; adds explicit Stream-callable negative cases |
| cross-unit Character grouping | unchanged |
| final presence reconciliation | unchanged |
| failure leaves previous state internally untouched | **clarified:** previous state is not returned/reported/consumed as current success; `fallback=false` |
| budgets | unchanged |
| accepted-root mutation test | **clarified:** mutation must alter an already included authoritative input; no new topology tag |

## 6. ProjectIndex/consumer delta

| Baseline behavior | Final behavior |
|---|---|
| source `content` entity owns root relations | manifest `ContentUnitId`/occurrence facts own them |
| PublicId-to-PublicId `ContentRoot` edge | typed ContentUnit -> occurrence -> target relations |
| Source root may project to index/bundle/watch/LSP | no accepted Source target, therefore no projection |
| consumer may refer to source-owned roots | consumer uses eight authored families plus Character/configured resource |
| Stream callable status unstated | callable never projects as content root |
| bundle/watch/LSP consume accepted inventory | unchanged and strengthened with no rescan/no partial publication |
| optional absent Character | unchanged: no symbol/payload, exact absence/watch fact |
| generated metadata/typed `res` references | unchanged authority; explicitly collected as typed facts under same revision |

## 7. Diagnostics delta

| Baseline expectation | Final expectation |
|---|---|
| Source could resolve as accepted family | deleted |
| removed Source spelling | ordinary unknown/wrong-family/wrong-symbol-kind diagnostic |
| Source-specific migration diagnostic | prohibited |
| callable returning Stream | ordinary wrong-symbol-kind diagnostic |
| alias/reexport | canonical original target and exact binding evidence; unchanged |
| collision/ambiguity/visibility/revision | unchanged |
| presence/Character diagnostics | unchanged |
| failed transaction | no partial or fallback success; clarified |

## 8. Test delta

The baseline Source-positive cases are deleted and replaced by:

- parser/HIR/sema/public/runtime/AWBC Source deletion tests;
- removed Source root produces no accepted target, ProjectIndex fact, bundle
  entry, watch input, LSP symbol, or compatibility node;
- ordinary Stream passthrough callable is not a root;
- authored Stream generator is not a root;
- external Stream capability operation is not a root;
- callable alias/reexport/name/return type cannot create a root;
- source `content` parser/compiler rejection and absence of executable typed
  node;
- direct manifest-owned ProjectIndex facts and consumer projections;
- no legacy type/variant alias compile tests.

Every non-Source Character, configured-resource, overlay, revision, presence,
collision, visibility, transaction, budget, and consumer test remains
authoritative and is represented in `TEST_MATRIX.md`.

## 9. Deletion/compatibility delta

Add to the baseline deletion inventory:

- every Source syntax/HIR/sema/public/runtime/AWBC/wire/consumer owner;
- old source-named content-root enum/variant;
- `EntityKind::Content`, `EntityDeclKind::Content`,
  `EntityDeclBody::Content`, `ContentDeclBody`;
- source-content ProjectIndex relation producer;
- Source/content LSP/bundle/watch compatibility projections.

The compatibility statement is strengthened to prohibit:

- Source family aliases/readers;
- replacement Source entities;
- function-name/return-type heuristics;
- provisional family tags;
- source-content compatibility nodes;
- fallback acceptance.

## 10. Unchanged safe-substrate summary

No row in this correction redesigns:

- manifest decoder/source map;
- binary/text overlay split;
- exact binary bytes;
- Character path/model/PNG validation;
- generated metadata validation;
- project containment;
- topology transcript/tag values;
- optional Character absence;
- resource registry digest;
- deterministic duplicate rejection;
- transaction-before-publication;
- budgets;
- Sans-I/O layering.

Those baseline decisions remain normative.
