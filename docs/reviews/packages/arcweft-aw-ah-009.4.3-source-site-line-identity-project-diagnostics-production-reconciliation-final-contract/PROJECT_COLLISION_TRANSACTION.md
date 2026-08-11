# Project collision transaction

## 1. Sole builder

```rust
pub struct HirProjectBuilder {
    root_package: CallablePackageId,
    modules: BTreeMap<HirPackageModuleKey, HirModuleSnapshot>,
}

impl HirProjectBuilder {
    pub fn new(root_package: CallablePackageId) -> Self;
    pub fn insert_module(
        &mut self,
        module: HirModuleSnapshot,
    ) -> Result<(), HirProjectBuildError>;
    pub fn finish(self) -> Result<HirProject, HirProjectBuildError>;
}
```

`insert_module` verifies package-qualified uniqueness and stores only scratch
state. `finish` is the only publication boundary. The old string-taking
`HirProject::new(package, modules)` is deleted after all callers migrate; no
wrapper or alias remains.

## 2. Input admission

Before line work, `finish` verifies:

- every `HirModuleKey` source equals its retained SourceDocument;
- package/module key consistency;
- exactly one root module for the root package;
- no duplicate package/module key;
- every included module is executable and has no error diagnostic;
- recovered modules have no candidate inventory;
- module, document, and aggregate source limits; and
- callable declaration package/module identity consistency.

Dependency and declaration-free modules use the same rules. An executable empty
module contributes an empty candidate slice.

## 3. Canonical traversal

The builder ignores insertion order. It iterates module `BTreeMap` order:

```text
(package, canonical module path)
```

Within a module it validates source order by:

```text
(application SourceSpan start,
 application SourceSpan end,
 DialogueLineSourceOrder,
 ExprId)
```

Any duplicate/torn source coordinate is an internal invariant failure.

## 4. Scratch state

```rust
struct DialogueLineAcceptanceTransaction {
    by_id: BTreeMap<DialogueLineId, PendingAcceptedSite>,
    accepted: Vec<AcceptedDialogueLine>,
    collisions: Vec<DialogueLineDiagnostic>,
    work: u32,
}
```

All state is local to `finish`. There is no global reservation service and no
mutation of module candidates.

## 5. Collision algorithm

For each candidate in canonical traversal:

1. charge one validation unit;
2. validate source identity against the module key and source registry;
3. look up its `DialogueLineId`;
4. if vacant, insert the site and append a pending accepted record;
5. if occupied, append AW-CD-020 with the current/later site primary and the
   originally inserted first site secondary; do not insert or alter either ID;
6. continue until input ends or a hard limit fails.

A third or later duplicate also relates to the original first site. This keeps
all permutations deterministic after canonical sorting.

## 6. Collision matrix

| First canonical site | Later site | Result |
|---|---|---|
| explicit | explicit | AW-CD-020 |
| explicit | generated | AW-CD-020; generated ordinal unchanged |
| generated | explicit | AW-CD-020; no renumbering |
| generated | generated | AW-CD-020; no probing |
| root module | dependency module | AW-CD-020 with two exact source identities |
| one document | another document | AW-CD-020 with cross-document secondary label |

Explicit/generated “either source order” is determined after canonical sorting,
not caller insertion order.

## 7. Diagnostic accumulation

Independent collisions are collected through all candidates up to 1,024
project line diagnostics and the fixed work limit. Diagnostics are sorted and
deduplicated by their complete typed key after traversal.

If the diagnostic capacity or work limit would be exceeded, `finish` returns a
fatal `DialogueLineProjectFatal::Limit` projected as AW-CD-025. Partial
collision diagnostics are not presented as a complete result and no project is
published.

## 8. Accepted output

```rust
pub struct AcceptedDialogueLine {
    id: DialogueLineId,
    text_key: DialogueTextKey,
    id_origin: DialogueLineIdOrigin,
    text_key_origin: DialogueTextKeyOrigin,
    source: AcceptedDialogueLineSource,
}

pub struct AcceptedDialogueLineSource {
    module: HirModuleKey,
    application: ExprId,
    owner: HirDialogueLineSourceOwner,
    named_scopes: Arc<[HirDialogueNamedScope]>,
    source_order: DialogueLineSourceOrder,
    application_span: SourceSpan,
    id_coordinate_span: Option<SourceSpan>,
    text_key_coordinate_span: Option<SourceSpan>,
}

pub struct AcceptedDialogueLineInventory {
    records: Arc<[AcceptedDialogueLine]>,
    by_id: BTreeMap<DialogueLineId, DialogueLineIndex>,
    by_expr: BTreeMap<ExprId, DialogueLineIndex>,
    source_order: Arc<[DialogueLineIndex]>,
}
```

Fields are private; constructors are crate-private and validate index
correlation. Public read-only accessors expose records, lookup by ID, lookup by
ExprId, and source-order iteration. `records` are sorted by line ID and then by
source key, although the second key is unreachable after uniqueness succeeds.

The inventory implements structural `Eq`; its crate-private canonical cache
bytes are length-prefixed and domain-separated. They are not a runtime/save
wire format.

## 9. Final publication

If collisions are nonempty, `finish` returns:

```rust
HirProjectBuildError::DialogueLines(DialogueLineProjectRejection)
```

The rejection owns the complete sorted diagnostics and no partial project.

If empty, indexes are finalized and the inventory is inserted into the one
immutable `HirProject`. `AcceptedProjectSnapshot::try_new` then consumes that
same project and performs its existing source/world/typecheck/semantic-index
transaction. It never reconstructs line facts.

## 10. Rollback proof

All project state is moved into `finish`; accepted generation publication
occurs only after it returns `Ok(HirProject)` and the existing accepted snapshot
transaction succeeds. Therefore rejection cannot:

- reserve an ID;
- advance an accepted generation;
- modify previous project/source/semantic caches;
- alter a subsequent valid build;
- publish a text key; or
- leave a partial line index visible.

## 11. Exact public API and errors

`AcceptedDialogueLine` and `AcceptedDialogueLineSource` derive
`Clone, Debug, Eq, PartialEq`; fields are private. Accessors expose every typed
field without allocating display strings. `AcceptedDialogueLineInventory`
derives `Clone, Debug, Eq, PartialEq` and has no public constructor.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineIndex(u32);

impl AcceptedDialogueLineInventory {
    pub fn records(&self) -> &[AcceptedDialogueLine];
    pub fn get(&self, id: &DialogueLineId) -> Option<&AcceptedDialogueLine>;
    pub fn for_expr(&self, expr: ExprId) -> Option<&AcceptedDialogueLine>;
    pub fn source_ordered(&self)
        -> impl ExactSizeIterator<Item = &AcceptedDialogueLine>;
}

impl HirProject {
    pub const fn root_package(&self) -> &CallablePackageId;
    pub fn dialogue_lines(&self) -> &AcceptedDialogueLineInventory;
}
```

Project errors are typed:

```rust
pub enum HirProjectBuildError {
    DuplicateModule { key: HirPackageModuleKey },
    MissingRootModule { package: CallablePackageId },
    ModulePackageMismatch { ... },
    SourceIdentityMismatch { ... },
    NonExecutableModule { key: HirModuleKey },
    CallableIdentityMismatch { ... },
    DialogueLines(DialogueLineProjectRejection),
    DialogueLineFatal(DialogueLineProjectFatal),
    // existing typed project-construction failures
}

pub struct DialogueLineProjectRejection {
    diagnostics: Arc<[DialogueLineDiagnostic]>,
}
```

`DialogueLineProjectRejection::diagnostics()` is public read-only. It contains
at least one complete collision diagnostic. Fatal limit/source/invariant errors
use `DialogueLineProjectFatal` and never masquerade as a complete rejection
set.
