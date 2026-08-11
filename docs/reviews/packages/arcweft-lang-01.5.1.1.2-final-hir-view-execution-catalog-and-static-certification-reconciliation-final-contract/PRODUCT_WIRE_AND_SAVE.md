# Product wire, codec, validation, and save contract

## Existing allocations retained

| Carrier | Allocation | Final action |
|---|---:|---|
| AWFB ViewProgram section/product codec | kind/tag 9 | keep |
| ViewProgram magic | `AWVP\r\n\x1a\n` | keep |
| common product schema | 1 | keep |
| required common field | 1 = canonical JSON transcript | keep, directly replace DTO |
| ViewText product codec | tag 11, `AWVT\r\n\x1a\n` | keep, replace stringly dynamic source variants |
| AWBC | ABI 1, codec tag 10 | keep |
| bundle session save | `arcweft.bundle_session`, version 2 | keep |

No new AWFB section, common field, codec tag, AWBC opcode/value tag, or save version
is allocated. The ViewProgram transcript is unreleased internal product data and is
replaced directly; old transcript bytes receive strict schema/unknown-field
rejection. No V2 wrapper or dual reader exists.

## Strict ViewProgram transcript

Field 1 contains a canonical compact JSON `ViewProgramTranscriptV1` with exact
root member order:

```text
schema
accepted_revision
program
value_programs
definitions
resolved_resources
static_fragments
static_certificates
source_map_digest
```

`schema` is exactly `arcweft.view_program.v1`. Arrays are sorted and unique by
their typed IDs; instruction order remains semantic order. Every object denies
unknown and duplicate fields. JSON numbers are used only for bounded integral
fields; digests/IDs use canonical lowercase hexadecimal or their existing strict
newtype encoding. Decoder re-encodes canonically and requires byte equality.

`WIRE_ALLOCATIONS.json` is the machine-readable allocation registry.

## Dynamic fields

A dynamic-capable field is encoded as `ViewBinding<T>`:

```json
{"kind":"constant","value":<native validated T>}
{"kind":"program","value":{"program":<id>,"projection":<typed tag>}}
```

Generic nested arguments and defaults are always `ViewValueProgramId` because
their value type is arbitrary `RuntimeCheckedType`. A literal argument is a
constant-returning ordinary AWBC function and can be eliminated by static
certification; it is never forced through `FxRuntimeValue`.

Replacements include:

- `ViewActionButtonResource.enabled: bool` -> `ViewBinding<bool>`;
- key `Option<ViewStableKey/u64>` -> `Option<ViewBinding<ViewStableKey>>`;
- text/local/projection strings -> typed Text source or RuntimeProgram;
- image ID/string selection -> `ViewImageBindingResource`;
- modifier/layout/scroll/navigation/input policies -> exact member contract plus
  native constant or projected program;
- handler/event strings and optional function binding -> typed event/handler IDs
  and required AWBC `CrossSectionRef`;
- nested argument ordinal/name authority -> exact `ViewParameterRef`;
- export ordinal -> typed program node/instruction/site/part coordinates plus an export contract digest.

Static-only fields are removed in the same direct schema switch; no shadow field is
kept.

## Decode and product validation order

1. Common envelope: length, magic, codec tag, schema, reserved bytes.
2. Field table: ordering, uniqueness, required field 1, no unknown field.
3. Transcript byte budget before JSON allocation.
4. Strict JSON, duplicate-key rejection, canonical re-encode equality.
5. `ViewProgramIdentity`, recomputed `AcceptedViewProgramRevision`, record ordering, uniqueness, and counts; no session generation is accepted.
6. Required AWBC function cross-section references, ABI 1, program digests,
   result types, role/input consistency.
7. Definition spans, instruction IDs/ranges, parameters/defaults, stable nested
   call targets, candidate-catalog parameter-table/per-parameter contract joins,
   branch/match/repeat/await ranges, locals, handlers, parts, and exports.
8. Binding projection versus exact property/type contract.
9. Resource identity triple, exact `ResourceTypeId`, declaration/descriptor/
   registry digests, image decode record, and generation.
10. ViewText/Input/Style/image/AWBC/source cross-section references.
11. Static subject, program-local coordinate bounds, dependency closure, immutable resources, fragment canonical bytes, semantic/program/certificate digests, and exact accepted revision.
12. Source references.
13. Construct `ValidatedViewProduct`; only then may bundle merge or runtime
    publication occur.

The first failing stage wins. Candidate merge state is discarded.

## Digests

All are BLAKE3-256 with an exact domain prefix and length-delimited canonical
fields:

```text
arcweft.view.semantic.v1\0
arcweft.view.value-semantic.v1\0
arcweft.view.dependency-closure.v1\0
arcweft.view.parameter-contract.v1\0
arcweft.view.parameter-table-contract.v1\0
arcweft.view.export-contract.v1\0
arcweft.view.static-fragment.v1\0
arcweft.view.static-certificate.v1\0
arcweft.view.program-semantic.v1\0
arcweft.view.program-revision.v1\0
```

Source spelling/ranges/roles, `ProductSourceId`, `SyntaxNodeId`, every HIR ID, and session generation are excluded. `CheckedViewCatalogGeneration` has no persisted digest or codec; it is compared structurally only inside the live semantic lease. Program-local coordinates are canonical typed integers scoped by `AcceptedViewProgramRevision`, not hashes.

## Save/replay

Session save remains schema version 2. No certificate, static fragment, selected
execution path, projected cache, renderer tree, or decoded image frame is saved.
The existing exact artifact identity binds the complete AWFB content root, including
the final ViewProgram and certificate bytes. `BundleViewRuntimeSnapshot.program_id`
binds the program; root/mount `RuntimeBinding`s already carry generic values;
`ViewMountSnapshot`, logical clock, allocator cursor, instance paths, initialized
parameter/state sets, input/focus state, and virtualization state remain the
semantic save payload.

Restore order:

1. outer save schema ID/version and strict payload;
2. artifact identity and active generation;
3. AWBC ABI and accepted ViewProgram ID/content root;
4. runtime catalog and certificate validation from the bound artifact;
5. mount graph/instance paths and owner identity;
6. parameter/local/runtime value type and nominal layout validation;
7. input/focus/state/virtualization and animation logical time;
8. resource references against active registry;
9. replay cursor and executor state;
10. atomic session publication.

A save produced against a different program/certificate/resource generation fails
closed. Static versus dynamic selection is re-derived from the bound validated
artifact, so both paths restore identical semantic state.
