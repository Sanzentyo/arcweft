# Diagnostics and deterministic failure order

## 1. Principle

Content-root diagnostics are ordinary typed-resolution, visibility, revision,
presence, and transaction diagnostics. Source elimination adds no
spelling-specific migration diagnostic.

The diagnostic owner receives typed candidates and exact `SourceSpan` evidence.
It does not infer meaning by scanning strings or repository files.

## 2. Resolution diagnostics

The following semantic codes are normative; existing repository naming may be
retained when it is one-to-one with these meanings.

| Code | Trigger | Primary span | Required secondary evidence |
|---|---|---|---|
| `InvalidContentRootReference` | manifest scalar cannot form the ordinary canonical reference | root `selection` | root `value` when useful |
| `UnknownContentRootFamily` | no final built-in category and no exact configured-resource category can resolve | root `selection` | accepted registry/world revision |
| `UnknownContentRootTarget` | family/category is valid but no exact target exists | root `selection` | family and accepted world revision |
| `WrongContentRootFamily` | exact entity exists but its typed `EntityKind` is not an accepted root family | root `selection` | declaration span and actual kind |
| `WrongContentRootSymbolKind` | path resolves to callable, nominal, module, external artifact, proof, or other non-entity/resource target | root `selection` | every candidate declaration/binding span and actual symbol category |
| `AmbiguousContentRootTarget` | more than one visible exact target remains | root `selection` | all candidates in canonical order |
| `ConfiguredResourceIdentityCollision` | resource declaration index contains duplicate canonical public identity | root `selection` or index construction span | all colliding declarations |
| `InaccessibleContentRootTarget` | exact target exists but accepted visibility denies manifest/package access | root `selection` | declaration and binding visibility spans |
| `ContentRootVisibilityEscalation` | content-unit visibility would publish a target more broadly than target visibility | content-unit `visibility` | root selection and target declaration visibility |
| `ContentRootWorldMismatch` | symbol/resource/reference world or revision differs from candidate topology | candidate/root span selected by mismatch owner | expected/actual world and revisions |
| `StaleContentRootOverlay` | root/reference/resource facts came from a stale text/binary overlay generation | stale occurrence span/path | accepted/current overlay identity |
| `UnconsumedContentBinaryOverlay` | explicitly supplied binary overlay was not consumed by exact topology | overlay path evidence | accepted resource inventory |

No diagnostic named `SourceRemoved`, `RemovedSourceRoot`,
`LegacySourceFamily`, or equivalent is permitted.

## 3. Former Source spelling

A reference that formerly named a Source declaration follows the ordinary
resolver:

- no final category/target: `UnknownContentRootFamily` or
  `UnknownContentRootTarget`, according to the resolver's ordinary typed
  distinction;
- a surviving callable at the same source-visible path:
  `WrongContentRootSymbolKind` with actual category `Callable`;
- a surviving entity of an invalid family: `WrongContentRootFamily`;
- ambiguous/inaccessible/stale cases: the corresponding ordinary diagnostic.

The message may quote the authored reference because that is source evidence.
It must not say or imply that a Source compatibility surface exists.

## 4. Stream callable cases

| Reference resolves to | Result |
|---|---|
| ordinary `fn -> Stream<T, E>` passthrough | `WrongContentRootSymbolKind(Callable)` |
| authored generator with own-scope `yield` | `WrongContentRootSymbolKind(Callable)` |
| external capability operation returning Stream | `WrongContentRootSymbolKind(Callable/External)` according to final symbol category |
| alias/reexport of any Stream callable | same wrong-kind result, canonical declaration shown |
| function whose name begins with `source`/`content` | same ordinary wrong-kind result |

Return type, generator mode, and external origin may appear as explanatory typed
details, but they do not select the diagnostic code.

## 5. Character and presence diagnostics

| Code | Trigger | Primary span/path |
|---|---|---|
| `RequiredRootMissing` | required Character manifest/package absent | root selection |
| `OptionalRootReferencedMissing(Profile)` | selected profile references absent optional Character | profile content-unit key/policy plus root |
| `OptionalRootReferencedMissing(Runtime)` | typed runtime/metadata/resource dependency references absent optional Character | first canonical reference occurrence |
| `OptionalRootReferencedMissing(ProfileAndRuntime)` | both causes | profile policy, then canonical runtime references |
| `CharacterIdentityMismatch` | manifest Character ID differs from root identity | Character manifest identity span/path |
| `CharacterLayerMissing` | manifest-named payload absent | manifest asset span/path |
| `CharacterLayerDuplicate` | duplicate canonical manifest asset path/payload seed | duplicate manifest spans or overlay paths |
| `CharacterLayerInvalidPng` | complete PNG decode fails | exact layer logical path |
| `CharacterLayerDimensionsMismatch` | decoded dimensions differ from manifest | exact layer path and expected/actual dimensions |
| `CharacterManifestSourceIdentityMismatch` | source-backed manifest/document identities differ | Character manifest source |
| `CharacterPackageCollision` | same Character identity maps to inconsistent package inputs | every conflicting root/package span |
| `UnreferencedCharacterLayerPayload` | explicitly supplied payload is outside typed manifest membership | overlay/dependency seed path |

Present-invalid always fails even for optional/unselected content.

## 6. Manifest/profile diagnostics

Existing strict decoder/profile diagnostics remain earlier than semantic root
resolution:

1. TOML syntax/schema/unknown field;
2. package/build/profile structural validation;
3. unknown content-unit selection/profile content policy;
4. duplicate/empty root array and exact source-map failures;
5. project containment/path normalization;
6. generated metadata hash/identity/ABI;
7. Character acquisition/validation;
8. typed symbol/resource world construction;
9. root resolution/visibility/revision;
10. reference and presence reconciliation;
11. consumer candidate construction.

A later stage cannot replace an earlier failure with a more convenient root
diagnostic.

## 7. Multiple diagnostic ordering

Within the root-resolution/finalization stage, sort by:

```text
manifest document identity
content unit ID
root ordinal
diagnostic class rank
canonical target identity
secondary source document identity
secondary source range
```

Reference occurrences sort by document identity and source range. Candidate
declarations sort by canonical identity then declaration range. Host directory
order, map insertion order, overlay seed order, and thread scheduling do not
affect output.

The existing maximum of 128 diagnostics applies. Truncation emits the existing
bounded-report marker and does not publish a partial accepted state.

## 8. Atomic diagnostic publication

A failed candidate publishes one failure report bound to the attempted
manifest/topology/source revision. It does not publish:

- new content facts;
- new ProjectIndex relations;
- new bundle/watch/LSP products;
- a previous accepted snapshot relabelled with the attempted revision.

Consumers may continue displaying the prior accepted snapshot only when their
host protocol explicitly distinguishes it as prior/stale state. It is never the
result of the failed admission and is never called final/fallback success.
